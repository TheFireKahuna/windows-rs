//! The arms that run children along one axis: stack, row and layer.
//!
//! A stack runs its children along the block axis and a row along the inline one, so each
//! distributes on the walk that owns its main axis — a row in **B**, a stack in **C** — and
//! aligns on the other. A layer overlaps its children in one box and distributes nothing.

use super::Solver;
use super::flow::{STRIDE, distribute};
use crate::layout::{Layout, WidthClass};
use windows_numerics::Vector2;
use windows_scene::NodeId;

// ── walk A ──────────────────────────────────────────────────────────────────────────

/// Answers the widest of this container's children, which is what a stack, a layer and a
/// scroll container take.
pub(crate) fn measure_max(s: &mut Solver<'_>, n: NodeId, inner: WidthClass) -> [f32; 2] {
    let mut pair = [0.0f32, 0.0f32];
    let mut c = s.first_flow(n);
    while !c.is_none() {
        let child = s.measure(c, inner);
        pair[0] = pair[0].max(child[0]);
        pair[1] = pair[1].max(child[1]);
        c = s.next_flow(c);
    }
    pair
}

/// Answers a row's summed children plus its gaps.
///
/// A wrapping row's minimum is its widest child rather than the sum, because it may break.
pub(crate) fn measure_row(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
    wraps: bool,
) -> [f32; 2] {
    let gap = s.gap(l, class, f32::NAN);
    let mut pair = [0.0f32, 0.0f32];
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if !s.hidden(c) {
            let child = s.measure(c, inner);
            pair[0] = if wraps {
                pair[0].max(child[0])
            } else {
                pair[0] + child[0]
            };
            pair[1] += child[1];
            count += 1;
        }
        c = s.next_flow(c);
    }
    let gaps = gap * count.saturating_sub(1) as f32;
    if !wraps {
        pair[0] += gaps;
    }
    pair[1] += gaps;
    pair
}

// ── walk B ──────────────────────────────────────────────────────────────────────────

/// Pushes one child's inline `[min, nat, grow, max]` onto the scratch.
pub(crate) fn push_inline(s: &mut Solver<'_>, c: NodeId, class: WidthClass, basis: f32) {
    let l = s.layout(c);
    let pair = s.inline_pair(c, class, basis);
    let max = s
        .len(l.max_width, class, basis)
        .unwrap_or(f32::INFINITY)
        .max(pair[0]);
    s.scratch
        .f
        .extend_from_slice(&[pair[0], pair[1], l.grow.max(0.0), max]);
}

/// Pushes one child's block `[min, nat, grow, max]` onto the scratch.
fn push_block(s: &mut Solver<'_>, c: NodeId, class: WidthClass, room: f32) {
    let l = s.layout(c);
    let pair = s.block_pair(c, class, room);
    let max = s
        .len(l.max_height, class, room)
        .unwrap_or(f32::INFINITY)
        .max(pair[0]);
    s.scratch
        .f
        .extend_from_slice(&[pair[0], pair[1], l.grow.max(0.0), max]);
}

/// Gives every child of a stack the container's width and answers the block pair they sum to.
pub(crate) fn place_stack(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
    iw: f32,
) -> [f32; 2] {
    let gap = s.gap(l, class, iw);
    // A scroll container's content does not hold its box open, so its minimum on the
    // scrolling axis is its own and not its children's sum.
    let scrolls = s.scrolls(n);
    let mut pair = [0.0f32, 0.0f32];
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        let cw = if s.hidden(c) {
            0.0
        } else {
            s.cross_inline(c, l.align, inner, iw)
        };
        let child = s.place(c, cw, inner);
        if !s.hidden(c) {
            pair[0] += child[0];
            pair[1] += child[1];
            count += 1;
        }
        c = s.next_flow(c);
    }
    let gaps = gap * count.saturating_sub(1) as f32;
    pair[0] += gaps;
    pair[1] += gaps;
    if scrolls {
        pair[0] = 0.0;
    }
    pair
}

/// Gives every child of a layer the container's width and answers their deepest block pair.
pub(crate) fn place_layer(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
    iw: f32,
) -> [f32; 2] {
    let _ = class;
    let mut pair = [0.0f32, 0.0f32];
    let mut c = s.first_flow(n);
    while !c.is_none() {
        let cw = if s.hidden(c) {
            0.0
        } else {
            s.cross_inline(c, l.align, inner, iw)
        };
        let child = s.place(c, cw, inner);
        pair[0] = pair[0].max(child[0]);
        pair[1] = pair[1].max(child[1]);
        c = s.next_flow(c);
    }
    pair
}

/// Distributes a row's inline room among its children and answers their tallest block pair.
pub(crate) fn place_row(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
    iw: f32,
) -> [f32; 2] {
    let gap = s.gap(l, class, iw);
    let base = s.scratch.f.len();
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.place(c, 0.0, inner);
        } else {
            push_inline(s, c, inner, iw);
            count += 1;
        }
        c = s.next_flow(c);
    }
    let gaps = gap * count.saturating_sub(1) as f32;
    let end = s.scratch.f.len();
    distribute(&mut s.scratch.f[base..end], (iw - gaps).max(0.0));
    let mut pair = [0.0f32, 0.0f32];
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if !s.hidden(c) {
            let w = s.scratch.f[base + k * STRIDE + 1];
            let child = s.place(c, w, inner);
            pair[0] = pair[0].max(child[0]);
            pair[1] = pair[1].max(child[1]);
            k += 1;
        }
        c = s.next_flow(c);
    }
    s.scratch.f.truncate(base);
    pair
}

// ── walk C ──────────────────────────────────────────────────────────────────────────

/// Distributes a stack's block room among its children and places each across it.
#[expect(clippy::too_many_arguments, reason = "one container's whole geometry")]
pub(crate) fn arrange_stack(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    inner: WidthClass,
    pad: Vector2,
    iw: f32,
    ih: f32,
    abs: Vector2,
) {
    let gap = s.gap(l, inner, iw);
    let base = s.scratch.f.len();
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if !s.hidden(c) {
            push_block(s, c, inner, ih);
            count += 1;
        }
        c = s.next_flow(c);
    }
    let gaps = gap * count.saturating_sub(1) as f32;
    let end = s.scratch.f.len();
    // A scroll container places its content unscrolled at its natural extent: clamping it to
    // the viewport is what scrolling exists to avoid.
    if !s.scrolls(n) {
        distribute(&mut s.scratch.f[base..end], (ih - gaps).max(0.0));
    }
    let mut used = gaps;
    for k in 0..count {
        used += s.scratch.f[base + k * STRIDE + 1];
    }
    let mut y = pad.y + l.justify.offset(ih, used);
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.arrange(c, Vector2::zero(), abs, 0.0, inner, true);
            c = s.next_flow(c);
            continue;
        }
        let ch = s.scratch.f[base + k * STRIDE + 1];
        let cw = s.geom(c).at_w;
        let held = s.layout(c);
        let x = pad.x + held.align_self_or(l.align).offset(iw, cw);
        s.arrange(c, Vector2::new(x, y), abs, ch, inner, false);
        y += ch + gap;
        k += 1;
        c = s.next_flow(c);
    }
    s.scratch.f.truncate(base);
}

/// Places a row's children along the widths walk B gave them.
#[expect(clippy::too_many_arguments, reason = "one container's whole geometry")]
pub(crate) fn arrange_row(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    inner: WidthClass,
    pad: Vector2,
    iw: f32,
    ih: f32,
    abs: Vector2,
) {
    let gap = s.gap(l, inner, iw);
    let mut used = 0.0f32;
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if !s.hidden(c) {
            used += s.geom(c).at_w;
            count += 1;
        }
        c = s.next_flow(c);
    }
    used += gap * count.saturating_sub(1) as f32;
    let mut x = pad.x + l.justify.offset(iw, used);
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.arrange(c, Vector2::zero(), abs, 0.0, inner, true);
            c = s.next_flow(c);
            continue;
        }
        let cw = s.geom(c).at_w;
        let ch = s.cross_block(c, l.align, inner, ih);
        let held = s.layout(c);
        let y = pad.y + held.align_self_or(l.align).offset(ih, ch);
        s.arrange(c, Vector2::new(x, y), abs, ch, inner, false);
        x += cw + gap;
        c = s.next_flow(c);
    }
}

/// Places every child of a layer in the container's own box.
///
/// The inline axis takes the container's `align` and the child's own exception to it; the
/// block axis takes `justify`, which a layer has no main axis to spend on.
#[expect(clippy::too_many_arguments, reason = "one container's whole geometry")]
pub(crate) fn arrange_layer(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    inner: WidthClass,
    pad: Vector2,
    iw: f32,
    ih: f32,
    abs: Vector2,
) {
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.arrange(c, Vector2::zero(), abs, 0.0, inner, true);
            c = s.next_flow(c);
            continue;
        }
        let cw = s.geom(c).at_w;
        let ch = s.cross_block(c, l.justify, inner, ih);
        let held = s.layout(c);
        let x = pad.x + held.align_self_or(l.align).offset(iw, cw);
        let y = pad.y + l.justify.offset(ih, ch);
        s.arrange(c, Vector2::new(x, y), abs, ch, inner, false);
        c = s.next_flow(c);
    }
}
