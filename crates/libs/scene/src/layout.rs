//! The taffy tree, the measure seam, and pixel snapping. **App half.**
//!
//! [`LayoutTree`] implements taffy's low-level tree traits, so flexbox, CSS grid, block
//! layout and the classifying container are all arms of one `compute_child_layout`. Styles
//! are [`taffy::Style`] with no conversion layer, and every per-node table is a `Vec`
//! indexed by the node's own dense id, so no node path hashes.
//!
//! The one layout mode this crate adds is [`LayoutKind::Responsive`]: a container that
//! classifies its own inline size into a [`WidthClass`] for its subtree.

use crate::id::Id;
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
    /// Left edge.
    pub x0: f32,
    /// Top edge.
    pub y0: f32,
    /// Right edge.
    pub x1: f32,
    /// Bottom edge.
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
}

/// One node's solved placement.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Solved {
    /// Absolute, window-relative, pixel-snapped and **unscrolled**: where layout placed the
    /// node, before any tracker offset.
    pub rect: Rect,
    /// Offset relative to the parent group, which is the value a visual's offset takes.
    pub local: Vector2,
    /// Size in DIPs, measured between the snapped edges of `rect`.
    pub size: Vector2,
    /// Content size in DIPs, which a scroll extent is derived from.
    pub content: Vector2,
    /// Whether the node clips or scrolls, and so confines its children.
    pub bounded: bool,
    /// The class the enclosing responsive container resolved for this node.
    ///
    /// Written for every node by the gather walk, so a re-lower running *outside* the solve
    /// reads the class from here rather than from a copy a class flip leaves behind.
    pub class: WidthClass,
}

/// Snaps a DIP coordinate onto the physical pixel grid at `scale`.
///
/// The root visual carries the display scale, so a coordinate that falls between physical
/// pixels makes the compositor bilinear-resample the whole surface and blurs text and
/// hairlines.
///
/// **Callers snap edges, never extents.** Snapping `x` and `x + w` keeps adjacent nodes
/// sharing an edge exactly; snapping `x` and `w` independently opens a hairline gap
/// between them at some scales and overlaps them at others.
///
/// A non-finite `v` returns `0.0`.
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
    /// Nothing to measure: the style determines the size.
    #[default]
    None,
    /// A fixed intrinsic size, in DIPs.
    Fixed(Vector2),
    /// Measured by the layer above, which owns the text engine. The key is opaque here.
    Measured(MeasureKey),
}

/// A measurement request's identity, minted and interpreted by the layer above.
///
/// This crate holds no text engine and no font ladder, so it never interprets a key. What it
/// decides is when the measurement runs and which [`WidthClass`] it runs under.
///
/// Generational like every other id, so a key held past the release of the run it named
/// misses rather than reading that slot's next occupant.
pub type MeasureKey = Id<crate::sink::Measured>;

/// How much room an axis has, in taffy's own three states.
///
/// The two intrinsic probes ask opposite questions, so a measurement answers each one
/// separately. Answering `MinContent` with the width a wrapping run occupies on one line
/// lets flex shrink the run below its own longest word, which breaks a word mid-way.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Avail {
    /// This much room, in DIPs.
    Definite(f32),
    /// The narrowest the content can be: the largest indivisible piece of it.
    MinContent,
    /// The width the content takes with nothing forced to break.
    MaxContent,
}

impl Avail {
    /// Returns the room in DIPs, or `None` for either intrinsic probe.
    ///
    /// For a measurement whose answer does not depend on which probe it is: a fixed-size
    /// leaf, or a single-line run that never breaks. A measurement that *can* break must
    /// match on the variant instead.
    #[must_use]
    pub const fn definite(self) -> Option<f32> {
        match self {
            Self::Definite(v) => Some(v),
            _ => None,
        }
    }
}

impl From<AvailableSpace> for Avail {
    fn from(space: AvailableSpace) -> Self {
        match space {
            AvailableSpace::Definite(v) => Self::Definite(v),
            AvailableSpace::MinContent => Self::MinContent,
            AvailableSpace::MaxContent => Self::MaxContent,
        }
    }
}

/// The inputs to one measurement.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MeasureIn {
    /// The request being answered, as the layer above minted it.
    pub key: MeasureKey,
    /// The class the enclosing responsive container resolved. Every metric a measurement
    /// depends on — type size, padding, tracking — resolves against this, so it is passed in
    /// rather than read from ambient state.
    pub class: WidthClass,
    /// Whichever dimensions layout has already fixed.
    pub known: (Option<f32>, Option<f32>),
    /// The room on each axis, and **which probe is being asked** where there is no number.
    pub available: (Avail, Avail),
}

/// Measures what this crate cannot: shaped text, and anything else whose size is content.
pub trait Measure: Send {
    /// Returns the size in DIPs of the content `input` names.
    fn measure(&mut self, input: MeasureIn) -> Vector2;
}

impl<F: FnMut(MeasureIn) -> Vector2 + Send> Measure for F {
    fn measure(&mut self, input: MeasureIn) -> Vector2 {
        self(input)
    }
}

/// Re-lowers a style whose metrics depend on the class in scope.
///
/// The twin of [`Measure`]: this crate resolves the class, and the layer above resolves what
/// a class *means*. Called during the solve, for the subtree of a container that just
/// changed class, so layout runs on the styles that class implies.
///
/// **Runs inside the solve**, so an implementation must not re-enter the model and must not
/// allocate.
pub trait Restyle: Send {
    /// Returns the style `node` takes at `class`, or `None` to leave its style alone.
    fn restyle(&mut self, node: NodeId, class: WidthClass) -> Option<Style>;
}

impl<F: FnMut(NodeId, WidthClass) -> Option<Style> + Send> Restyle for F {
    fn restyle(&mut self, node: NodeId, class: WidthClass) -> Option<Style> {
        self(node, class)
    }
}

/// How a node lays its children out.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum LayoutKind {
    /// Lays children out by `style.display`.
    #[default]
    Container,
    /// Classifies its own inline size for its subtree, then delegates.
    ///
    /// **Its inline size must be determined by its parent.** A content-sized container has
    /// nothing to classify: it arrives at the `PerformLayout` pass with no known inline
    /// size, where a debug assertion fires.
    Responsive(Bounds),
}

#[derive(Debug)]
struct LayoutNode {
    /// This slot's own id. A `TaffyId` is a bare index and carries no generation, so it
    /// cannot name a node back to the layer above.
    id: NodeId,
    style: Style,
    kind: LayoutKind,
    measure: MeasureCtx,
    children: Vec<TaffyId>,
    /// The node to walk up to when this one is dirtied. Taffy caches a node's output keyed
    /// on its *input*, so a change here that leaves a parent's input alone — hiding a
    /// child, re-measuring a leaf — leaves the parent's cached answer standing.
    parent: Option<TaffyId>,
    cache: Cache,
    /// The class this node last resolved, for the hysteresis band. Unused on anything but a
    /// responsive node.
    class: WidthClass,
    /// Hidden independently of `style.display`.
    ///
    /// A flag of its own rather than a `Display::None` written into the style keeps hiding
    /// reversible without recording what the display *was*: a hidden grid comes back a
    /// grid, and a style re-pushed while hidden does not reveal the node.
    hidden: bool,
    unrounded: Layout,
    solved: Layout,
    live: bool,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            id: NodeId::default(),
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

/// The layout tree, persistent across passes.
///
/// A node and its style live from [`create`](Self::create) to [`destroy`](Self::destroy), so
/// a pass neither rebuilds them nor discards the taffy cache that makes an unchanged subtree
/// cost nothing.
pub struct LayoutTree {
    nodes: Vec<LayoutNode>,
    /// The class in scope while a subtree is being laid out. Written by a responsive node
    /// before it delegates; read by every descendant's measurement.
    ambient: WidthClass,
    measure: Option<Box<dyn Measure>>,
    restyle: Option<Box<dyn Restyle>>,
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
    /// Returns an empty tree, with no measure and no restyle callback installed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            ambient: WidthClass::default(),
            measure: None,
            restyle: None,
        }
    }

    /// Installs the callback that measures content-sized nodes, replacing any previous one.
    pub fn on_measure(&mut self, measure: impl Measure + 'static) {
        self.measure = Some(Box::new(measure));
    }

    /// Installs the callback that re-lowers a class-dependent style, replacing any previous
    /// one.
    pub fn on_restyle(&mut self, restyle: impl Restyle + 'static) {
        self.restyle = Some(Box::new(restyle));
    }

    /// Clears `id`'s cache and every cache above it.
    ///
    /// Taffy keys a cached layout on the input it was given, so a subtree change that leaves
    /// a parent's input alone — hiding a child, re-measuring a leaf, reordering siblings —
    /// is answered from the parent's cache unless the walk up clears it.
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
            id: node,
            kind,
            live: true,
            // The `Vec` keeps its allocation: ids are dense and reused, so the next mint
            // into this slot fills it again.
            children: core::mem::take(&mut slot.children),
            ..LayoutNode::default()
        };
        slot.children.clear();
        self.mark_dirty(TaffyId::from(node.index()));
    }

    /// Declares how a node lays its children out, leaving the rest of the slot alone, and
    /// returns whether the kind differed.
    ///
    /// Separate from [`create`](Self::create), which resets the slot: setting the kind
    /// composes in any order with pushing a style and setting children.
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

    /// Pushes a style, and returns whether it differed from the one in place.
    ///
    /// An equal style is dropped rather than pushed, so re-stating a style does not
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

    /// Hides a node without removing it, so its state and its subtree survive, and returns
    /// whether the flag changed.
    ///
    /// The flag is the node's own, never a `Display::None` written into its style: the style
    /// carries what the node *is*, so a hidden grid comes back a grid and a style re-pushed
    /// while hidden does not reveal the node.
    pub fn set_hidden(&mut self, node: NodeId, hidden: bool) -> bool {
        let slot = self.slot(node);
        if slot.hidden == hidden {
            return false;
        }
        slot.hidden = hidden;
        self.mark_dirty(TaffyId::from(node.index()));
        true
    }

    /// Declares where a node's intrinsic size comes from.
    pub fn set_measure(&mut self, node: NodeId, ctx: MeasureCtx) {
        let slot = self.slot(node);
        if slot.measure != ctx {
            slot.measure = ctx;
            self.mark_dirty(TaffyId::from(node.index()));
        }
    }

    /// Marks a measured node's **input** as moved, though its measure context did not
    /// change.
    ///
    /// A run's [`MeasureKey`] is stable for the run's whole life, so a label whose string
    /// changed pushes an identical [`MeasureCtx`] and
    /// [`set_measure`](Self::set_measure) reports no change. Every other input to a layout
    /// is a field this type holds and compares — a style, a hidden flag, a child list — and
    /// a measurement's input is not, so its owner is the only one that can invalidate it.
    pub fn remeasure(&mut self, node: NodeId) {
        self.mark_dirty(TaffyId::from(node.index()));
    }

    /// Replaces a node's children, in paint order.
    ///
    /// **Every child is pointed back at `node`, including when the list compares equal.** A
    /// `TaffyId` is a bare index, so a list of ids reused at the same positions compares
    /// equal to one whose members are fresh nodes that [`create`](Self::create) reset,
    /// parent included. A child left with no parent stops
    /// [`mark_dirty`](Self::mark_dirty) at itself, and every ancestor keeps a cache that
    /// predates the change.
    pub fn set_children(&mut self, node: NodeId, children: &[NodeId]) {
        let parent = TaffyId::from(node.index());
        for &child in children {
            self.slot(child).parent = Some(parent);
        }
        let slot = self.slot(node);
        if slot.children.len() == children.len()
            && core::iter::zip(&slot.children, children)
                .all(|(&a, b)| a == TaffyId::from(b.index()))
        {
            return;
        }
        slot.children.clear();
        slot.children
            .extend(children.iter().map(|c| TaffyId::from(c.index())));
        self.mark_dirty(parent);
    }

    /// Empties `out` and sizes it to one entry per node.
    ///
    /// Called once per pass, not once per root: the attached tree and every detached root
    /// gather into the same buffer, indexed by node id.
    pub fn begin(&mut self, out: &mut Vec<Solved>) {
        out.clear();
        out.resize(self.nodes.len(), Solved::default());
    }

    /// Solves the tree under `root` against a window of `size` DIPs, placing it at `origin`,
    /// and writes each node's placement into `out`, indexed by node id.
    ///
    /// [`begin`](Self::begin) must have sized `out` for this pass; the gather indexes it
    /// directly.
    ///
    /// Snapping happens here, at the end, in one place — and against the *absolute* rect,
    /// which is the space adjacent edges have to agree in.
    ///
    /// `origin` is zero for the window's own root and is the resolved placement for a
    /// detached one. It **translates and does not constrain**: the subtree is laid out
    /// against `size` wherever it lands, so an overlay's size does not depend on its
    /// position. Because the origin is an input to the gather, a detached subtree's
    /// [`Solved::rect`] is absolute, which is the space the hit array is scanned in.
    pub fn solve_root(
        &mut self,
        root: NodeId,
        size: Vector2,
        origin: Vector2,
        scale: f32,
        out: &mut Vec<Solved>,
    ) {
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

        let (ox, oy) = (snap(origin.x, scale), snap(origin.y, scale));
        self.gather(taffy_root, ox, oy, scale, WidthClass::default(), out);
        // A root has no parent to position it, so its offset within one is the origin it was
        // placed at. Taffy leaves that at zero; writing it here makes the placement reach the
        // compositor as the same offset bind every other node's does.
        if let Some(solved) = out.get_mut(root.index()) {
            solved.local = Vector2 { x: ox, y: oy };
        }
    }

    fn gather(
        &self,
        id: TaffyId,
        ox: f32,
        oy: f32,
        scale: f32,
        class: WidthClass,
        out: &mut Vec<Solved>,
    ) {
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
            class,
        };
        // A responsive container classifies its size *for its subtree*: its own style was
        // lowered at the enclosing class, so the class changes on the way down to the
        // children and not for this node.
        let inner = match node.kind {
            LayoutKind::Responsive(_) => node.class,
            LayoutKind::Container => class,
        };
        for &child in &node.children {
            self.gather(child, x, y, scale, inner, out);
        }
    }

    fn node(&self, id: TaffyId) -> &LayoutNode {
        &self.nodes[usize::from(id)]
    }

    fn node_mut(&mut self, id: TaffyId) -> &mut LayoutNode {
        &mut self.nodes[usize::from(id)]
    }

    /// Re-lowers `id` at `class` and clears its cache, then descends.
    ///
    /// Taffy keys a cached layout on its *input* and the ambient class is not in that key,
    /// so a descendant whose own inputs did not change keeps the measurement it took under
    /// the previous class unless its cache is cleared.
    ///
    /// The descent **stops at a nested responsive container**: that node's own style
    /// resolves at the class enclosing it, which is `class`, while its subtree resolves at
    /// the class it classifies for itself once the cleared cache makes it lay out again.
    fn reclass(&mut self, id: TaffyId, class: WidthClass) {
        self.node_mut(id).cache.clear();
        // Taken out for the call and put back, so the callback does not borrow the tree the
        // node lookups around it borrow.
        if let Some(mut restyle) = self.restyle.take() {
            let node = self.node(id).id;
            if let Some(style) = restyle.restyle(node, class) {
                self.node_mut(id).style = style;
            }
            self.restyle = Some(restyle);
        }
        if matches!(self.node(id).kind, LayoutKind::Responsive(_)) {
            return;
        }
        for index in 0..self.node(id).children.len() {
            let child = self.node(id).children[index];
            self.reclass(child, class);
        }
    }

    fn measure_leaf(&mut self, id: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        let (style, ctx, class) = {
            let node = self.node(id);
            (node.style.clone(), node.measure, self.ambient)
        };
        // Taken out for the call and put back, so the closure handed to
        // `compute_leaf_layout` holds the callback directly and borrows no part of the tree.
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
                                available: (available.width.into(), available.height.into()),
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
        // Committed on the `PerformLayout` pass only. Taffy probes a subtree at widths it may
        // not lay it out at, and a flip consumed by a probe would leave `PerformLayout` with
        // no transition to re-lower against.
        if inputs.run_mode == RunMode::PerformLayout && class != previous {
            self.node_mut(id).class = class;
            for index in 0..self.node(id).children.len() {
                let child = self.node(id).children[index];
                self.reclass(child, class);
            }
        }

        // Scoped to the subtree: saving and restoring `ambient` leaves a sibling laid out
        // afterwards reading the class *its* own enclosing container resolved.
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
        // Rounding is this crate's own: taffy rounds to whole *DIPs*, and a snapped edge has
        // to land on a whole physical pixel, which at a fractional scale is not a DIP
        // boundary.
        node.solved = *layout;
    }

    fn compute_child_layout(&mut self, id: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let node = tree.node(id);
            // A hidden pass stays hidden the whole way down. Taffy descends into a hidden
            // subtree with `RunMode::PerformHiddenLayout` and does not call a measure
            // function in that mode, so a node deciding by its *own* display alone would
            // hand the first childless descendant of a hidden container to `measure_leaf`
            // and reach taffy's `unreachable!()`. The run mode is the only input that names
            // which pass this is.
            if inputs.run_mode == RunMode::PerformHiddenLayout
                || node.hidden
                || node.style.display == Display::None
            {
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

    /// Solves one attached root at the origin, which is every case here but the detached one.
    impl LayoutTree {
        fn solve(&mut self, root: NodeId, size: Vector2, scale: f32, out: &mut Vec<Solved>) {
            self.begin(out);
            self.solve_root(root, size, Vector2 { x: 0.0, y: 0.0 }, scale, out);
        }
    }

    #[test]
    fn snapping_keeps_adjacent_edges_exactly_shared() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            // Two boxes meeting at a fractional edge: snapping the shared coordinate, not
            // each box's extent, keeps them meeting.
            let edge = snap(37.3, scale);
            assert_eq!(snap(37.3, scale), edge);
            let px = edge * scale;
            assert!((px - px.round()).abs() < 1.0e-3);
        }
    }

    fn tree_with_a_row() -> (LayoutTree, NodeId, [NodeId; 3]) {
        let mut tree = LayoutTree::new();
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
        // The style carries the node's display and the hidden flag is separate. Folding the
        // two together brings a hidden grid back as a flexbox.
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
        // The widget layer re-states a node's style on every rebuild, so a style push must
        // not clear the hidden flag.
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
        // Setting the kind composes in any order with pushing a style and setting children.
        // Re-creating the slot to set it drops both, leaving a subtree that is not laid out.
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
        // Idempotent, so re-declaring the kind does not dirty the tree.
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
        // Cross-axis stretch is flexbox's default and would give every child the
        // container's height, hiding the measured height this test reads.
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
        tree.set_measure(leaf, MeasureCtx::Measured(MeasureKey::raw(7, 1)));
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

    #[test]
    fn a_node_minted_after_a_solve_reaches_the_next_one() {
        // The two solves a screen switch runs: structure and a solve, then a publish that
        // mints a wrapping run's line sprites, and a second solve. The publish's parent was
        // rebuilt into the slot its predecessor held, so its own parent's child list
        // compares equal, and only the re-link keeps that parent climbable when the mint
        // dirties upward.
        let mut tree = LayoutTree::new();
        let column = Style {
            display: Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            ..Style::DEFAULT
        };
        let root = NodeId::raw(1, 1);
        let mid = NodeId::raw(2, 1);
        let window = Vector2 { x: 600.0, y: 400.0 };
        let origin = Vector2 { x: 0.0, y: 0.0 };
        let mut out = Vec::new();

        for node in [root, mid] {
            tree.create(node, LayoutKind::Container);
            tree.set_style(node, &column);
        }
        tree.set_style(
            root,
            &Style {
                size: Size {
                    width: percent(1.0_f32),
                    height: percent(1.0_f32),
                },
                ..column.clone()
            },
        );
        tree.set_children(root, &[mid]);

        // The first screen: a group under `mid`, with one line sprite published into it.
        let mut build =
            |tree: &mut LayoutTree, group: NodeId, line: NodeId, out: &mut Vec<Solved>| {
                tree.create(group, LayoutKind::Container);
                tree.set_style(group, &column);
                tree.set_children(mid, &[group]);
                tree.begin(out);
                tree.solve_root(root, window, origin, 1.0, out);
                // Minted by the publish, which runs after that solve and takes its box from it.
                tree.create(line, LayoutKind::Container);
                tree.set_style(
                    line,
                    &Style {
                        size: Size {
                            width: length(47.0_f32),
                            height: length(16.0_f32),
                        },
                        ..Style::DEFAULT
                    },
                );
                tree.set_children(group, &[line]);
                tree.begin(out);
                tree.solve_root(root, window, origin, 1.0, out);
            };

        let (group, line) = (NodeId::raw(3, 1), NodeId::raw(4, 1));
        build(&mut tree, group, line, &mut out);
        assert_eq!(out[line.index()].size, Vector2 { x: 47.0, y: 16.0 });

        // Navigating away and back. Both slots come back at the same dense indices, so
        // `mid`'s child list is unchanged by inspection and the group's is too.
        tree.destroy(line);
        tree.destroy(group);
        build(&mut tree, NodeId::raw(3, 2), NodeId::raw(4, 2), &mut out);
        assert_eq!(
            out[4].size,
            Vector2 { x: 47.0, y: 16.0 },
            "a node minted after the solve was never reached by the next one"
        );
    }

    #[test]
    fn a_detached_root_gathers_absolutely_at_its_origin() {
        // The window subtree and the detached one share the output buffer, so a second root
        // must not clear the first, and the detached root's rect has to be in the same
        // absolute space the one hit array is scanned in.
        let (mut tree, root, kids) = tree_with_a_row();
        let window = Vector2 { x: 600.0, y: 400.0 };

        let overlay = NodeId::raw(5, 1);
        let item = NodeId::raw(6, 1);
        tree.create(overlay, LayoutKind::Container);
        tree.create(item, LayoutKind::Container);
        tree.set_style(
            item,
            &Style {
                size: Size {
                    width: length(120.0_f32),
                    height: length(30.0_f32),
                },
                ..Style::DEFAULT
            },
        );
        tree.set_children(overlay, &[item]);

        let mut out = Vec::new();
        tree.begin(&mut out);
        tree.solve_root(root, window, Vector2 { x: 0.0, y: 0.0 }, 1.0, &mut out);
        tree.solve_root(
            overlay,
            window,
            Vector2 { x: 210.0, y: 64.0 },
            1.0,
            &mut out,
        );

        // The window subtree survived the second root.
        assert_eq!(out[kids[1].index()].rect.x0, 100.0);
        // The overlay sized itself to its content rather than to the window it was measured
        // against, and landed where it was placed.
        let solved = out[overlay.index()];
        assert_eq!(solved.rect.x0, 210.0);
        assert_eq!(solved.rect.y0, 64.0);
        assert_eq!(solved.size, Vector2 { x: 120.0, y: 30.0 });
        // A root's offset within its parent is the origin it was placed at, so the
        // placement travels as the ordinary offset bind.
        assert_eq!(solved.local, Vector2 { x: 210.0, y: 64.0 });
        // And its children are absolute in the same space.
        assert_eq!(out[item.index()].rect.x0, 210.0);
        assert_eq!(out[item.index()].local, Vector2 { x: 0.0, y: 0.0 });
    }

    /// A container, its child, and a nested container with a child of its own.
    fn nested_containers() -> (LayoutTree, NodeId, [NodeId; 4]) {
        let mut tree = LayoutTree::new();
        let root = NodeId::raw(1, 1);
        let outer = NodeId::raw(2, 1);
        let child = NodeId::raw(3, 1);
        let inner = NodeId::raw(4, 1);
        let grandchild = NodeId::raw(5, 1);
        tree.create(root, LayoutKind::Container);
        tree.create(outer, LayoutKind::Responsive(Bounds([600.0, 1000.0])));
        tree.create(child, LayoutKind::Container);
        tree.create(inner, LayoutKind::Responsive(Bounds([200.0, 400.0])));
        tree.create(grandchild, LayoutKind::Container);
        // Column, so each child gets the container's full inline size. In a row they would
        // share it, and the nested container would classify against a width this test did
        // not set.
        let stack = Style {
            display: Display::Flex,
            flex_direction: taffy::FlexDirection::Column,
            size: Size {
                width: percent(1.0_f32),
                height: percent(1.0_f32),
            },
            ..Style::DEFAULT
        };
        for id in [root, outer, child, grandchild] {
            tree.set_style(id, &stack);
        }
        // Fixed, so the nested container's own class is constant across the outer flip and
        // this test measures only what the outer flip did.
        tree.set_style(
            inner,
            &Style {
                size: Size {
                    width: length(300.0_f32),
                    height: percent(1.0_f32),
                },
                ..stack
            },
        );
        tree.set_children(root, &[outer]);
        tree.set_children(outer, &[child, inner]);
        tree.set_children(inner, &[grandchild]);
        (tree, root, [outer, child, inner, grandchild])
    }

    #[test]
    fn a_nodes_solved_class_is_the_one_its_own_style_was_lowered_at() {
        // A container classifies its size *for its subtree*. Its own style resolved at the
        // class enclosing it, so reporting its own class back would re-lower its padding at
        // the class it hands down.
        let (mut tree, root, [outer, child, inner, grandchild]) = nested_containers();
        let mut out = Vec::new();
        tree.solve(root, Vector2 { x: 300.0, y: 400.0 }, 1.0, &mut out);

        assert_eq!(out[root.index()].class, WidthClass::default());
        assert_eq!(
            out[outer.index()].class,
            WidthClass::default(),
            "a container reported the class it resolved rather than the one it sits in"
        );
        // 300 DIPs is Narrow against [600, 1000], and that governs the subtree.
        assert_eq!(out[child.index()].class, WidthClass::Narrow);
        assert_eq!(out[inner.index()].class, WidthClass::Narrow);
        // The nested one is 300 wide against [200, 400] — Medium — for its own subtree.
        assert_eq!(out[grandchild.index()].class, WidthClass::Medium);
    }

    #[test]
    fn a_flip_re_lowers_the_subtree_and_stops_at_a_nested_container() {
        // The nested container's own style resolves at the enclosing class, so it is
        // restyled; its subtree resolves at its own, which it re-resolves for itself.
        let (mut tree, root, [outer, child, inner, grandchild]) = nested_containers();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&seen);
        tree.on_restyle(move |node: NodeId, class: WidthClass| {
            record.lock().unwrap().push((node, class));
            None
        });

        let mut out = Vec::new();
        // Wide first: 1400 against [600, 1000].
        tree.solve(
            root,
            Vector2 {
                x: 1400.0,
                y: 400.0,
            },
            1.0,
            &mut out,
        );
        seen.lock().unwrap().clear();
        // Then Narrow, which flips `outer`.
        tree.solve(root, Vector2 { x: 300.0, y: 400.0 }, 1.0, &mut out);

        let calls = seen.lock().unwrap();
        let nodes: Vec<NodeId> = calls.iter().map(|(n, _)| *n).collect();
        assert!(nodes.contains(&child), "a subtree node was not re-lowered");
        assert!(
            nodes.contains(&inner),
            "a nested container's own style was not re-lowered"
        );
        assert!(
            !nodes.contains(&grandchild),
            "the walk descended past a nested container into a subtree it does not govern"
        );
        assert!(
            !nodes.contains(&outer),
            "a container re-lowered its own style at the class it hands down"
        );
        assert!(
            calls.iter().all(|(_, c)| *c == WidthClass::Narrow),
            "the subtree was re-lowered at a class other than the one just resolved"
        );
    }
}
