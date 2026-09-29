//! Three walks over the node arena, each visiting a node at most once.
//!
//! **A** answers one `(minimum, natural)` inline pair per node, bottom-up. **B** hands every
//! node the width it gets and answers the block pair that width implies. **C** hands it the
//! height and the origin, snaps its four edges and publishes the box. Widths never depend on
//! heights, so no subtree is visited twice; the one exception is a responsive container whose
//! class flips, which re-measures its own subtree under the new class inside the same pass.
//!
//! A dirty bit, not a comparison, decides what is visited: **B** returns at once where the
//! node is clean and the width handed down has not moved, and **C** returns where the node is
//! clean and its box lands exactly where it already is. A window resize that moves nothing
//! below a fixed-size container therefore costs nothing below it.

mod flow;
mod grid;
mod linear;

#[cfg(test)]
mod tests;

use crate::build::Host;
use crate::build::tree::{self, Geom, Tree};
use crate::layout::{Align, Bounds, Edge, Layout, Len, Position, Preset, Rect, WidthClass};
use crate::role::Scope;
use core::cell::RefCell;
use windows_numerics::Vector2;
use windows_scene::{Forest, NO_LINK, NodeId};

/// Snaps a DIP coordinate onto the physical pixel grid at `scale`.
///
/// **Callers snap edges, never extents.** Snapping `x` and `x + w` keeps adjacent nodes
/// sharing an edge exactly; snapping `x` and `w` independently opens a hairline gap between
/// them at some scales and overlaps them at others.
///
/// A non-finite `v` returns `0.0`.
#[must_use]
pub fn snap(v: f32, scale: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    (v * scale).round() / scale
}

/// Snaps a trailing edge onto the pixel grid without ever moving it inward.
///
/// For a box whose content re-flows against its own width, rounding the far edge to the
/// nearest pixel is not neutral: a run measured to need 133.56 DIPs published in 133.33 breaks
/// a second line inside a box one line tall, and nothing about the result reads as a rounding
/// fault. Rounding to nearest and taking the next pixel where that landed short holds the
/// content by at most one pixel and needs no tolerance.
#[must_use]
fn hold(v: f32, scale: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    let px = v * scale;
    let at = px.round();
    if at < px { at + 1.0 } else { at }.max(0.0) / scale
}

/// Solves one root: the window root, or a detached overlay root.
///
/// The root is measured against the window, takes the extent its own declaration asks for,
/// and is arranged at the origin its last placement left in `Geom::local`.
pub fn solve_root(host: &mut Host, root: NodeId) {
    #[cfg(test)]
    ROOTS.with(|held| held.set(held.get() + 1));
    with_solver(host, root, |s| {
        let class = s.tree.class(root);
        let window = s.window;
        let pair = s.measure(root, class);
        let at = s.geom(root).at;
        let w = s.root_inline(root, class, pair, window.x);
        let pair_h = s.place(root, w, class);
        let h = s.root_block(root, class, pair_h, window.y);
        let hidden = s.hidden(root);
        s.arrange(root, at, Vector2::zero(), h, class, hidden);
    });
}

/// Re-publishes a placed root's subtree boxes at a new origin, without measuring anything.
///
/// Overlay placement translates and does not constrain, so the subtree's sizes and pairs are
/// unchanged and only the boxes move. Every edge is snapped again, because a translate by a
/// fractional offset lands the subtree on a different pixel grid.
pub fn shift(host: &mut Host, root: NodeId, at: Vector2) {
    with_solver(host, root, |s| {
        let hidden = s.hidden(root);
        s.translate(root, at, Vector2::zero(), hidden);
    });
}

/// Runs `f` against a solver over `host`, returning the scratch for the next pass.
fn with_solver(host: &mut Host, root: NodeId, f: impl FnOnce(&mut Solver<'_>)) {
    if !host.tree.is_live(root) {
        return;
    }
    let window = host.window_extent();
    let scale = host.env.scale();
    let scope = host.scope_of(root);
    let scratch = SCRATCH.with_borrow_mut(core::mem::take);
    let mut s = Solver {
        tree: &mut host.tree,
        text: &mut host.text,
        rows: &host.metrics,
        scratch,
        scale,
        scope,
        window,
        visits: 0,
    };
    f(&mut s);
    let scratch = core::mem::take(&mut s.scratch);
    SCRATCH.with_borrow_mut(|held| *held = scratch);
}

/// Depth-indexed working room for the walks.
///
/// Stacks rather than a buffer per container: each level takes a window at the top, uses it
/// and truncates, so the capacity reaches its high-water mark once and the walks allocate
/// nothing after that.
#[derive(Default)]
pub(crate) struct Scratch {
    /// Track bases, wrap line extents and distribution bookkeeping.
    pub f: Vec<f32>,
    /// Grid occupancy words and per-line item counts.
    pub u: Vec<u16>,
    /// The subtree a class flip marks.
    pub marks: Vec<NodeId>,
}

thread_local! {
    /// The walks' scratch, kept between passes so the second flush allocates nothing.
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

#[cfg(test)]
thread_local! {
    /// How many nodes walk B entered rather than returned clean from.
    static VISITS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
    /// How many roots were solved.
    static ROOTS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
    /// How many times the walks asked the text table for a measurement.
    static ASKS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

/// Returns how many nodes the walks placed since the last call, and resets the count.
#[cfg(test)]
pub(crate) fn take_visits() -> u32 {
    VISITS.with(|held| held.replace(0))
}

/// Returns how many roots were solved since the last call, and resets the count.
#[cfg(test)]
pub(crate) fn take_roots() -> u32 {
    ROOTS.with(|held| held.replace(0))
}

/// Returns how many text measurements the walks asked for, and resets the count.
#[cfg(test)]
pub(crate) fn take_asks() -> u32 {
    ASKS.with(|held| held.replace(0))
}

/// One pass's borrows of the arena and the stores the walks read through.
pub(crate) struct Solver<'a> {
    pub tree: &'a mut Tree,
    pub text: &'a mut crate::build::text::Table,
    pub rows: &'a [[f32; crate::role::BUILTIN_METRICS]; 3],
    pub scratch: Scratch,
    pub scale: f32,
    /// The scope every metric resolves against, so the walk and the host's cached table
    /// cannot answer differently for one metric.
    pub scope: Scope,
    /// The window's client extent, which every root is measured against.
    pub window: Vector2,
    pub visits: u32,
}

impl Drop for Solver<'_> {
    fn drop(&mut self) {
        #[cfg(test)]
        VISITS.with(|held| held.set(held.get() + self.visits));
    }
}

impl Solver<'_> {
    // ── column access ───────────────────────────────────────────────────────────────

    pub(crate) fn layout(&self, n: NodeId) -> Layout {
        self.tree.c.layout[n.index()]
    }

    pub(crate) fn geom(&self, n: NodeId) -> Geom {
        self.tree.c.geom[n.index()]
    }

    fn bits(&self, n: NodeId) -> tree::Bits {
        self.tree.c.flags[n.index()]
    }

    pub(crate) fn dirty(&self, n: NodeId) -> bool {
        self.bits(n) & (tree::MEASURE | tree::DESC) != 0
    }

    pub(crate) fn hidden(&self, n: NodeId) -> bool {
        self.bits(n) & tree::HIDDEN != 0
    }

    fn responsive(&self, n: NodeId) -> bool {
        self.bits(n) & tree::RESPONSIVE != 0
    }

    /// Returns whether this container's block axis holds its content open rather than
    /// confining it. A scroll container's content does not hold its box open.
    pub(crate) fn scrolls(&self, n: NodeId) -> bool {
        self.bits(n) & tree::SCROLL != 0
    }

    /// Returns whether the walks lay this node out.
    ///
    /// A derived sprite carries its own geometry and takes no room from its parent, so all
    /// three walks skip it and its box is published from its parent's.
    fn laid_out(&self, n: NodeId) -> bool {
        self.bits(n) & tree::DERIVED == 0
    }

    // ── child iteration ─────────────────────────────────────────────────────────────

    /// This node's first laid-out child, bottom to top.
    pub(crate) fn first(&self, n: NodeId) -> NodeId {
        self.laid_out_from(self.tree.links(n.index() as u32).first)
    }

    /// The next laid-out sibling above `c`.
    pub(crate) fn next(&self, c: NodeId) -> NodeId {
        self.laid_out_from(self.tree.links(c.index() as u32).next)
    }

    fn laid_out_from(&self, mut at: u32) -> NodeId {
        while at != NO_LINK {
            let id = self.tree.id_at(at);
            if self.laid_out(id) {
                return id;
            }
            at = self.tree.links(at).next;
        }
        NodeId::NONE
    }

    /// This node's first in-flow child, which is the one a container arranges.
    pub(crate) fn first_flow(&self, n: NodeId) -> NodeId {
        self.flow_from(self.first(n))
    }

    pub(crate) fn next_flow(&self, c: NodeId) -> NodeId {
        self.flow_from(self.next(c))
    }

    fn flow_from(&self, mut c: NodeId) -> NodeId {
        while !c.is_none() {
            if self.layout(c).position.in_flow() {
                return c;
            }
            c = self.next(c);
        }
        c
    }

    // ── length resolution ───────────────────────────────────────────────────────────

    pub(crate) fn len(&self, l: Len, class: WidthClass, basis: f32) -> Option<f32> {
        l.resolve(self.rows, class, self.scope, basis, self.scale)
    }

    /// Returns the total padding this node adds on each axis.
    ///
    /// None for a text leaf: a single line paints into the node's own sprite and the coverage
    /// brush fills it, so a box wider than the ink would smear the glyphs across the padding.
    pub(crate) fn padding(&self, l: &Layout, class: WidthClass, basis: f32) -> (f32, f32) {
        if l.preset == Preset::Text {
            return (0.0, 0.0);
        }
        let x = self.len(l.padding[0], class, basis).unwrap_or(0.0);
        let y = self.len(l.padding[1], class, basis).unwrap_or(0.0);
        (x * 2.0, y * 2.0)
    }

    pub(crate) fn gap(&self, l: &Layout, class: WidthClass, basis: f32) -> f32 {
        self.len(l.gap, class, basis).unwrap_or(0.0)
    }

    /// Returns `pair` clamped by this node's authored inline minimum and maximum.
    fn clamp_inline(&self, l: &Layout, class: WidthClass, mut pair: [f32; 2]) -> [f32; 2] {
        if let Some(max) = self.len(l.max_width, class, f32::NAN) {
            pair[0] = pair[0].min(max);
            pair[1] = pair[1].min(max);
        }
        // An authored minimum raises a derived floor and never lowers it, so it is applied
        // after the maximum and wins where the two disagree.
        if let Some(min) = self.len(l.min_width, class, f32::NAN) {
            pair[0] = pair[0].max(min);
            pair[1] = pair[1].max(min);
        }
        pair[1] = pair[1].max(pair[0]);
        pair
    }

    /// Returns `pair` clamped by this node's authored block minimum, maximum and floor.
    fn clamp_block(&self, l: &Layout, class: WidthClass, mut pair: [f32; 2]) -> [f32; 2] {
        if let Some(max) = self.len(l.max_height, class, f32::NAN) {
            pair[0] = pair[0].min(max);
            pair[1] = pair[1].min(max);
        }
        let stated = self.len(l.min_height, class, f32::NAN);
        // The floor applies where the author stated neither a height nor a minimum height, so
        // a control is as tall as its author said and keeps its accessible minimum otherwise.
        let least = match stated {
            Some(min) => Some(min),
            None if !l.height.is_set() => self.len(l.floor, class, f32::NAN),
            None => None,
        };
        if let Some(min) = least {
            pair[0] = pair[0].max(min);
            pair[1] = pair[1].max(min);
        }
        pair[1] = pair[1].max(pair[0]);
        pair
    }

    /// Returns the width a root takes against a window of `window` DIPs.
    fn root_inline(&self, n: NodeId, class: WidthClass, pair: [f32; 2], window: f32) -> f32 {
        let l = self.layout(n);
        let held = self
            .len(l.width, class, window)
            .unwrap_or_else(|| pair[1].min(window));
        self.clamp_inline(&l, class, [held, held])[1]
    }

    /// Returns the height a root takes against a window of `window` DIPs.
    fn root_block(&self, n: NodeId, class: WidthClass, pair: [f32; 2], window: f32) -> f32 {
        let l = self.layout(n);
        let held = self
            .len(l.height, class, window)
            .unwrap_or_else(|| pair[1].min(window));
        self.clamp_block(&l, class, [held, held])[1]
    }

    /// Returns the arrangement this node runs, which a row states against the width class.
    pub(crate) fn preset(&self, l: &Layout, class: WidthClass) -> Preset {
        if l.preset.inline_main() && l.stacks_at(class) {
            return Preset::Stack;
        }
        l.preset
    }

    /// Returns the class this node's children resolve against.
    fn inner_class(&self, n: NodeId, class: WidthClass) -> WidthClass {
        if self.responsive(n) {
            return self.tree.own_class(n);
        }
        class
    }

    // ── walk A ──────────────────────────────────────────────────────────────────────

    /// Answers this node's `[minimum, natural]` inline widths under `class`.
    pub(crate) fn measure(&mut self, n: NodeId, class: WidthClass) -> [f32; 2] {
        if self.hidden(n) {
            return [0.0, 0.0];
        }
        if !self.dirty(n) {
            let g = self.geom(n);
            return [g.pair[0], g.pair[1]];
        }
        let l = self.layout(n);
        let preset = self.preset(&l, class);
        let inner = self.inner_class(n, class);
        let content = match preset {
            Preset::Figure => {
                linear::measure_max(self, n, inner);
                [0.0, 0.0]
            }
            Preset::Text => {
                linear::measure_max(self, n, inner);
                self.measure_text(n, class)
            }
            Preset::Row => linear::measure_row(self, n, &l, class, inner, false),
            Preset::Wrap => linear::measure_row(self, n, &l, class, inner, true),
            Preset::Grid => grid::measure(self, n, &l, class, inner),
            Preset::Stack | Preset::Layer | Preset::Scroll => linear::measure_max(self, n, inner),
        };
        let (pad_x, _) = self.padding(&l, class, f32::NAN);
        let mut pair = if l.width.is_pct() {
            // Resolved in B, against the width its container hands down.
            [0.0, 0.0]
        } else if let Some(v) = self.len(l.width, class, f32::NAN) {
            [v, v]
        } else if let Some(v) = self.aspect_inline(&l, class) {
            [v, v]
        } else if self.responsive(n) {
            // Its width is its parent's to decide, and its content was just measured under
            // the class that width will replace, so the content's extent stays here: what
            // walk B hands down decides the class, and the children are measured again
            // under it where it moved.
            [0.0, 0.0]
        } else {
            [content[0] + pad_x, content[1] + pad_x]
        };
        pair = self.clamp_inline(&l, class, pair);
        let g = &mut self.tree.c.geom[n.index()];
        g.pair[0] = pair[0];
        g.pair[1] = pair[1];
        pair
    }

    fn measure_text(&mut self, n: NodeId, class: WidthClass) -> [f32; 2] {
        #[cfg(test)]
        ASKS.with(|held| held.set(held.get() + 1));
        let key = self.tree.c.text[n.index()];
        if key == crate::build::text::MeasureKey::NONE {
            return [0.0, 0.0];
        }
        self.text.pair(key, class)
    }

    /// Returns the width an aspect ratio derives from a stated height.
    fn aspect_inline(&self, l: &Layout, class: WidthClass) -> Option<f32> {
        if l.aspect <= 0.0 {
            return None;
        }
        Some(self.len(l.height, class, f32::NAN)? * l.aspect)
    }

    // ── walk B ──────────────────────────────────────────────────────────────────────

    /// Gives this node the width `w` and answers the `[minimum, natural]` heights it implies.
    pub(crate) fn place(&mut self, n: NodeId, w: f32, class: WidthClass) -> [f32; 2] {
        let i = n.index();
        if self.hidden(n) {
            let g = &mut self.tree.c.geom[i];
            g.at_w = 0.0;
            g.pair[2] = 0.0;
            g.pair[3] = 0.0;
            return [0.0, 0.0];
        }
        if !self.dirty(n) && self.geom(n).at_w == w {
            let g = self.geom(n);
            return [g.pair[2], g.pair[3]];
        }
        self.visits += 1;
        self.tree.set_class(n, class);
        self.tree.c.flags[i] |= tree::PLACED;
        self.tree.c.geom[i].at_w = w;
        let l = self.layout(n);
        let inner = self.classify(n, &l, w, class);
        let preset = self.preset(&l, class);
        let (pad_x, pad_y) = self.padding(&l, class, w);
        let iw = (w - pad_x).max(0.0);
        let content = match preset {
            // A figure fills its parent and a text leaf is its own ink, so neither takes its
            // extent from a child; a child either carries its own geometry or overlaps.
            Preset::Figure => {
                linear::place_layer(self, n, &l, class, inner, iw);
                [0.0, 0.0]
            }
            Preset::Text => {
                linear::place_layer(self, n, &l, class, inner, iw);
                let h = self.place_text(n, class, iw);
                [h, h]
            }
            Preset::Row => linear::place_row(self, n, &l, class, inner, iw),
            Preset::Wrap => flow::place_wrap(self, n, &l, class, inner, iw),
            Preset::Grid => grid::place(self, n, &l, class, inner, iw),
            Preset::Layer => linear::place_layer(self, n, &l, class, inner, iw),
            Preset::Stack | Preset::Scroll => linear::place_stack(self, n, &l, class, inner, iw),
        };
        self.place_out_of_flow(n, inner, w);
        let mut pair = if l.height.is_pct() {
            [0.0, 0.0]
        } else if let Some(v) = self.len(l.height, class, f32::NAN) {
            [v, v]
        } else if l.aspect > 0.0 {
            let v = w / l.aspect;
            [v, v]
        } else {
            [content[0] + pad_y, content[1] + pad_y]
        };
        pair = self.clamp_block(&l, class, pair);
        let g = &mut self.tree.c.geom[i];
        g.pair[2] = pair[0];
        g.pair[3] = pair[1];
        pair
    }

    fn place_text(&mut self, n: NodeId, class: WidthClass, w: f32) -> f32 {
        #[cfg(test)]
        ASKS.with(|held| held.set(held.get() + 1));
        let key = self.tree.c.text[n.index()];
        if key == crate::build::text::MeasureKey::NONE {
            return 0.0;
        }
        self.text.height_at(key, class, w)
    }

    /// Classifies a responsive container's own width and answers the class its subtree takes.
    ///
    /// On a flip the subtree is marked so walk A takes its pairs again under the new class:
    /// every cached measurement is keyed on the class that produced it, and a descendant
    /// whose own inputs did not change would otherwise keep the pair it measured under the
    /// previous class.
    ///
    /// The container's own width is parent-determined and nothing inside it can change that,
    /// so the re-measure terminates with no fixed point to search for.
    fn classify(&mut self, n: NodeId, l: &Layout, w: f32, class: WidthClass) -> WidthClass {
        if !self.responsive(n) {
            return class;
        }
        #[cfg(debug_assertions)]
        {
            let parent = self.tree.parent(n);
            let content_sized = self.tree.is_live(parent)
                && self.preset(&self.layout(parent), class).inline_main()
                && !l.width.is_set()
                && l.grow <= 0.0;
            debug_assert!(
                !content_sized,
                "a responsive container's inline size must be determined by its parent"
            );
        }
        let held = self.tree.own_class(n);
        let next = Bounds(l.bounds).reclassify(w, held);
        if next != held {
            self.tree.set_own_class(n, next);
            let mut stack = core::mem::take(&mut self.scratch.marks);
            self.tree.mark_subtree(n, &mut stack);
            stack.clear();
            self.scratch.marks = stack;
            let mut c = self.first_flow(n);
            while !c.is_none() {
                self.measure(c, next);
                c = self.next_flow(c);
            }
        }
        next
    }

    // ── walk C ──────────────────────────────────────────────────────────────────────

    /// Gives this node the height `h` at `local` inside its parent, and publishes its box.
    pub(crate) fn arrange(
        &mut self,
        n: NodeId,
        local: Vector2,
        parent: Vector2,
        h: f32,
        class: WidthClass,
        hidden: bool,
    ) {
        let i = n.index();
        let hidden = hidden || self.hidden(n);
        let (local, w, h) = if hidden {
            (Vector2::zero(), 0.0, 0.0)
        } else {
            (local, self.geom(n).at_w, h)
        };
        let abs = Vector2::new(parent.x + local.x, parent.y + local.y);
        // A run breaks its lines against the box it is published at, and the height solved for
        // it answers the width that box was asked for. The two have to be the same width.
        let holds = self.layout(n).preset == Preset::Text;
        let rect = self.box_at(abs, w, h, holds);
        let held = self.geom(n);
        let flags = &mut self.tree.c.flags[i];
        // A box that is only shown or only hidden keeps its rect, so the bit is what says
        // its derived sprites moved.
        let sunk = *flags & tree::SUNK != 0;
        *flags = if hidden { *flags | tree::SUNK } else { *flags & !tree::SUNK };
        let placed = *flags & tree::PLACED != 0;
        let settled = !self.dirty(n) && self.published(n) && sunk == hidden;
        if settled && !placed && rect == held.rect && local == held.at {
            return;
        }
        if settled && h == held.at_h && !placed {
            // Only the origin moved, so every descendant keeps its offset and its extent.
            self.translate(n, local, parent, hidden);
            return;
        }
        self.publish(n, local, parent, h, rect);
        self.tree.c.flags[i] &= !(tree::MEASURE | tree::DESC | tree::PLACED);
        if hidden {
            let mut c = self.first(n);
            while !c.is_none() {
                self.arrange(c, Vector2::zero(), abs, 0.0, class, true);
                c = self.next(c);
            }
            self.settle_derived(n);
            return;
        }
        let l = self.layout(n);
        let inner = self.inner_class(n, class);
        let preset = self.preset(&l, class);
        let (pad_x, pad_y) = self.padding(&l, class, w);
        let pad = Vector2::new(pad_x * 0.5, pad_y * 0.5);
        let iw = (w - pad_x).max(0.0);
        let ih = (h - pad_y).max(0.0);
        match preset {
            Preset::Figure | Preset::Text => {
                linear::arrange_layer(self, n, &l, inner, pad, iw, ih, abs);
            }
            Preset::Row => linear::arrange_row(self, n, &l, inner, pad, iw, ih, abs),
            Preset::Wrap => flow::arrange_wrap(self, n, &l, inner, pad, iw, ih, abs),
            Preset::Grid => grid::arrange(self, n, &l, inner, pad, iw, ih, abs),
            Preset::Layer => linear::arrange_layer(self, n, &l, inner, pad, iw, ih, abs),
            Preset::Stack | Preset::Scroll => {
                linear::arrange_stack(self, n, &l, inner, pad, iw, ih, abs);
            }
        }
        self.arrange_out_of_flow(n, inner, w, h, abs);
        self.settle_derived(n);
    }

    /// Returns the snapped box a node of `w` by `h` occupies at `abs`.
    ///
    /// `holds` keeps the far edges from rounding inward, for a box whose content re-flows
    /// against its own width. See [`hold`].
    fn box_at(&self, abs: Vector2, w: f32, h: f32, holds: bool) -> Rect {
        let (x0, y0) = (snap(abs.x, self.scale), snap(abs.y, self.scale));
        if holds {
            // Held from the snapped origin: an origin rounded forward would otherwise take up
            // to half a pixel out of the extent the far edge holds.
            return Rect::new(x0, y0, hold(x0 + w, self.scale), hold(y0 + h, self.scale));
        }
        Rect::new(
            x0,
            y0,
            snap(abs.x + w, self.scale),
            snap(abs.y + h, self.scale),
        )
    }

    /// Writes one node's published box and records it for the encode.
    ///
    /// The offset is the box's own origin against the parent's box, which is where the
    /// parent's visual sits. Snapping the offset by itself would put the visual a pixel off
    /// the box wherever the parent's fraction and the child's round apart.
    fn publish(&mut self, n: NodeId, at: Vector2, parent: Vector2, h: f32, rect: Rect) {
        let scale = self.scale;
        let g = &mut self.tree.c.geom[n.index()];
        g.rect = rect;
        g.at = at;
        g.at_h = h;
        g.local = Vector2::new(rect.x0 - snap(parent.x, scale), rect.y0 - snap(parent.y, scale));
        g.size = Vector2::new(rect.width(), rect.height());
        self.tree.touch(n);
    }

    /// Returns whether the encode has ever written this node's box.
    ///
    /// A minted slot's published row carries `NaN`, so a node that has never crossed is
    /// arranged even where its box lands exactly where the previous occupant's did.
    fn published(&self, n: NodeId) -> bool {
        !self.tree.c.published[n.index()].local.x.is_nan()
    }

    /// Re-publishes a subtree's boxes at a new origin.
    ///
    /// Every box is snapped again from the origin and extent the arrange used, not from the
    /// snapped box it produced: a snapped offset added to a moved parent rounds differently
    /// from the sum it stands for.
    fn translate(&mut self, n: NodeId, local: Vector2, parent: Vector2, hidden: bool) {
        let hidden = hidden || self.hidden(n);
        let abs = Vector2::new(parent.x + local.x, parent.y + local.y);
        let held = self.geom(n);
        let (w, h) = if hidden { (0.0, 0.0) } else { (held.at_w, held.at_h) };
        let rect = self.box_at(abs, w, h, self.layout(n).preset == Preset::Text);
        self.publish(n, local, parent, h, rect);
        self.tree.c.flags[n.index()] &= !(tree::MEASURE | tree::DESC | tree::PLACED);
        let mut c = self.first(n);
        while !c.is_none() {
            let at = self.geom(c).at;
            self.translate(c, at, abs, hidden);
            c = self.next(c);
        }
        self.settle_derived(n);
    }

    /// Clears the marks on the derived sprites the walks skip and hands them their parent's
    /// [`SUNK`](tree::SUNK) bit.
    ///
    /// Their boxes are published from their parent's, and a mark left standing here would
    /// leave a node claiming its input is unsolved after the publication ends. A derived
    /// sprite carries its own rect, so a hidden parent's zero box alone would not stop it
    /// painting.
    fn settle_derived(&mut self, n: NodeId) {
        let sunk = self.tree.c.flags[n.index()] & tree::SUNK;
        let mut at = self.tree.links(n.index() as u32).first;
        while at != NO_LINK {
            let id = self.tree.id_at(at);
            at = self.tree.links(at).next;
            if self.laid_out(id) {
                continue;
            }
            let flags = &mut self.tree.c.flags[id.index()];
            *flags = (*flags & !(tree::MEASURE | tree::DESC | tree::SUNK)) | sunk;
            self.settle_derived(id);
        }
    }

    // ── out of flow ─────────────────────────────────────────────────────────────────

    /// Gives every out-of-flow child its width, against the parent's own box.
    fn place_out_of_flow(&mut self, n: NodeId, class: WidthClass, w: f32) {
        let mut c = self.first(n);
        while !c.is_none() {
            let held = self.layout(c);
            if !held.position.in_flow() {
                // Walk A skips an out-of-flow child, because it takes no room from its
                // siblings, so this is where its subtree is measured.
                let pair = self.measure(c, class);
                let cw = self.out_of_flow_inline(&held, class, w, pair);
                self.place(c, cw, class);
            }
            c = self.next(c);
        }
    }

    fn out_of_flow_inline(&self, l: &Layout, class: WidthClass, w: f32, pair: [f32; 2]) -> f32 {
        match l.position {
            Position::Pin(edge) if edge.horizontal() => {
                self.len(l.width, class, w).unwrap_or(pair[1]).min(w)
            }
            Position::Pin(_) | Position::Band { .. } => self.len(l.width, class, w).unwrap_or(w),
            Position::Anchor { at, align } => {
                let room = ((at[2] - at[0]) * w).max(0.0);
                if align[0] == Align::Stretch {
                    self.len(l.width, class, room).unwrap_or(room)
                } else {
                    self.len(l.width, class, room).unwrap_or(pair[1]).min(w)
                }
            }
            Position::Flow | Position::Cell { .. } => w,
        }
    }

    /// Places every out-of-flow child against the parent's final box.
    ///
    /// An anchored child's pull-back comes from the extent the solve measured for it, and it
    /// is applied before the edges are snapped, so the hit array and the clip read the one
    /// box walk C writes.
    fn arrange_out_of_flow(&mut self, n: NodeId, class: WidthClass, w: f32, h: f32, abs: Vector2) {
        let mut c = self.first(n);
        while !c.is_none() {
            let held = self.layout(c);
            if !held.position.in_flow() {
                let (local, ch) = self.out_of_flow_box(c, &held, class, w, h);
                self.arrange(c, local, abs, ch, class, false);
            }
            c = self.next(c);
        }
    }

    fn out_of_flow_box(
        &mut self,
        c: NodeId,
        l: &Layout,
        class: WidthClass,
        w: f32,
        h: f32,
    ) -> (Vector2, f32) {
        let held = self.geom(c);
        let cw = held.at_w;
        let nat = held.pair[3];
        match l.position {
            Position::Pin(edge) => {
                let ch = if edge.horizontal() {
                    h
                } else {
                    self.len(l.height, class, h).unwrap_or(nat)
                };
                let x = if edge == Edge::Right { w - cw } else { 0.0 };
                let y = if edge == Edge::Bottom { h - ch } else { 0.0 };
                (Vector2::new(x, y), ch)
            }
            Position::Band { at } => {
                let y = self.len(at, class, h).unwrap_or(0.0);
                let ch = self.len(l.height, class, h).unwrap_or(nat);
                (Vector2::new(0.0, y), ch)
            }
            Position::Anchor { at, align } => {
                let region = Rect::new(at[0] * w, at[1] * h, at[2] * w, at[3] * h);
                let ch = if align[1] == Align::Stretch {
                    self.len(l.height, class, region.height())
                        .unwrap_or(region.height())
                } else {
                    self.len(l.height, class, h).unwrap_or(nat)
                };
                let x = match align[0].fraction() {
                    Some(f) => region.x0 + f * (region.width() - cw),
                    None => region.x0,
                };
                let y = match align[1].fraction() {
                    Some(f) => region.y0 + f * (region.height() - ch),
                    None => region.y0,
                };
                (Vector2::new(x, y), ch)
            }
            Position::Flow | Position::Cell { .. } => (Vector2::zero(), h),
        }
    }

    // ── shared child geometry ───────────────────────────────────────────────────────

    /// Returns the inline pair walk A wrote for this node, with a percentage resolved.
    ///
    /// Walks B and C read this rather than calling [`Solver::measure`]: A already ran over
    /// every in-flow node, and re-entering it from each level would measure a node once per
    /// level of the depth above it.
    pub(crate) fn inline_pair(&mut self, c: NodeId, class: WidthClass, room: f32) -> [f32; 2] {
        let l = self.layout(c);
        if l.width.is_pct() {
            let v = self.len(l.width, class, room).unwrap_or(0.0);
            return self.clamp_inline(&l, class, [v, v]);
        }
        if self.hidden(c) {
            return [0.0, 0.0];
        }
        let g = self.geom(c);
        [g.pair[0], g.pair[1]]
    }

    /// Returns the inline extent a container of `room` gives one cross-axis child.
    ///
    /// A child that stretches takes the room; one that does not takes its natural width,
    /// clipped by the room and floored at its own minimum. A `NO_STRETCH` leaf never
    /// stretches, which is what lets a label's box be its ink on the first solve.
    pub(crate) fn cross_inline(
        &mut self,
        c: NodeId,
        container: Align,
        class: WidthClass,
        room: f32,
    ) -> f32 {
        let l = self.layout(c);
        let pair = self.inline_pair(c, class, room);
        let stretches = l.width.is_auto()
            && l.align_self_or(container) == Align::Stretch
            && !l.has(Layout::NO_STRETCH);
        let want = if stretches { room } else { pair[1].min(room) };
        let max = self
            .len(l.max_width, class, room)
            .unwrap_or(f32::INFINITY)
            .max(pair[0]);
        want.max(pair[0]).min(max)
    }

    /// Returns the block pair a child offers inside a container of `room`.
    ///
    /// A percentage height resolves here, against the room its container ended up with.
    pub(crate) fn block_pair(&mut self, c: NodeId, class: WidthClass, room: f32) -> [f32; 2] {
        let l = self.layout(c);
        if l.height.is_pct()
            && let Some(v) = self.len(l.height, class, room)
        {
            return self.clamp_block(&l, class, [v, v]);
        }
        let g = self.geom(c);
        [g.pair[2], g.pair[3]]
    }

    /// Returns the block extent a container of `room` gives one cross-axis child.
    pub(crate) fn cross_block(
        &mut self,
        c: NodeId,
        container: Align,
        class: WidthClass,
        room: f32,
    ) -> f32 {
        let l = self.layout(c);
        let pair = self.block_pair(c, class, room);
        let stretches = l.height.is_auto()
            && l.aspect <= 0.0
            && l.align_self_or(container) == Align::Stretch
            && !l.has(Layout::NO_STRETCH);
        let want = if stretches { room } else { pair[1] };
        let max = self
            .len(l.max_height, class, room)
            .unwrap_or(f32::INFINITY)
            .max(pair[0]);
        want.max(pair[0]).min(max)
    }
}
