//! The shrink contract, and the arm that breaks a row into lines.
//!
//! Every in-flow item offers a `(minimum, natural)` pair on the axis it runs along, and
//! [`distribute`] is the one place those pairs become extents: if the naturals fit, growers
//! take the surplus by weight from their natural basis; if they do not, items give ground
//! proportionally between natural and minimum; what is still over runs past the container and
//! is clipped there.

use super::{Solver, linear};
use crate::layout::{Layout, WidthClass};
use windows_numerics::Vector2;
use windows_scene::NodeId;

/// How many scratch slots one item takes: minimum, natural, grow weight, maximum.
pub(crate) const STRIDE: usize = 4;

/// Resolves one axis's extents in place, by the shrink contract.
///
/// `items` holds one `[min, nat, grow, max]` group per item, and the natural slot is replaced
/// by the extent that item gets. `room` is what is left once the gaps are taken out.
pub(crate) fn distribute(items: &mut [f32], room: f32) {
    let n = items.len() / STRIDE;
    if n == 0 {
        return;
    }
    let mut natural = 0.0;
    let mut least = 0.0;
    let mut weight = 0.0;
    for k in 0..n {
        least += items[k * STRIDE];
        natural += items[k * STRIDE + 1];
        weight += items[k * STRIDE + 2];
    }
    if natural > room {
        let deficit = natural - room;
        let slack = natural - least;
        if slack <= 0.0 || deficit >= slack {
            for k in 0..n {
                items[k * STRIDE + 1] = items[k * STRIDE];
            }
            return;
        }
        for k in 0..n {
            let min = items[k * STRIDE];
            let nat = items[k * STRIDE + 1];
            items[k * STRIDE + 1] = nat - deficit * ((nat - min) / slack);
        }
        return;
    }
    let mut surplus = room - natural;
    if weight <= 0.0 || surplus <= 0.0 {
        return;
    }
    // One correction round: a grower whose share would carry it past its own maximum takes
    // only up to it and returns the rest to the others.
    let mut live = weight;
    for k in 0..n {
        let g = items[k * STRIDE + 2];
        if g <= 0.0 || live <= 0.0 {
            continue;
        }
        let nat = items[k * STRIDE + 1];
        let max = items[k * STRIDE + 3];
        if nat + surplus * (g / live) > max {
            items[k * STRIDE + 1] = max;
            items[k * STRIDE + 2] = 0.0;
            surplus -= max - nat;
            live -= g;
        }
    }
    if live <= 0.0 || surplus <= 0.0 {
        return;
    }
    for k in 0..n {
        let g = items[k * STRIDE + 2];
        if g > 0.0 {
            items[k * STRIDE + 1] += surplus * (g / live);
        }
    }
}

/// Gives every child of a wrapping row its width and answers the block pair the lines imply.
pub(crate) fn place_wrap(
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
            linear::push_inline(s, c, inner, iw);
            count += 1;
        }
        c = s.next_flow(c);
    }
    let mut at = 0usize;
    let mut cursor = visible(s, s.first_flow(n));
    let mut block = 0.0f32;
    let mut lines = 0usize;
    while at < count {
        // Lines break on the naturals, so distributing inside one line cannot move the break
        // that follows it, and walk C groups the children exactly as this walk did.
        let end = line_end(&s.scratch.f[base..], at, count, iw, gap);
        let gaps = gap * (end - at - 1) as f32;
        let from = base + at * STRIDE;
        let to = base + end * STRIDE;
        distribute(&mut s.scratch.f[from..to], (iw - gaps).max(0.0));
        let mut line_h = 0.0f32;
        for k in at..end {
            let w = s.scratch.f[base + k * STRIDE + 1];
            let pair = s.place(cursor, w, inner);
            line_h = line_h.max(pair[1]);
            cursor = visible(s, s.next_flow(cursor));
        }
        block += line_h;
        lines += 1;
        at = end;
    }
    s.scratch.f.truncate(base);
    let total = block + gap * lines.saturating_sub(1) as f32;
    [total, total]
}

/// Returns one past the last item that fits on the line starting at `at`.
///
/// A line always takes its first item, however wide, so the walk cannot stall and a lone
/// oversize item is the only one on its line.
fn line_end(items: &[f32], at: usize, count: usize, iw: f32, gap: f32) -> usize {
    let mut used = items[at * STRIDE + 1];
    let mut end = at + 1;
    while end < count {
        let next = items[end * STRIDE + 1];
        if used + gap + next > iw {
            break;
        }
        used += gap + next;
        end += 1;
    }
    end
}

/// Places every child of a wrapping row, line by line.
#[expect(clippy::too_many_arguments, reason = "one container's whole geometry")]
pub(crate) fn arrange_wrap(
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
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.arrange(c, Vector2::zero(), abs, 0.0, inner, true);
        }
        c = s.next_flow(c);
    }
    let mut y = pad.y;
    let mut start = visible(s, s.first_flow(n));
    while !start.is_none() {
        // The same break rule walk B used: the natural width, not the extent an item shrank
        // to, which would otherwise let it join the line before it.
        // A line is as tall as its tallest child wants to be, not as tall as the container:
        // the container's own height is what these lines added up to.
        let mut used = basis(s, start, inner, iw);
        let mut line_h = s.block_pair(start, inner, ih)[1];
        let mut end = visible(s, s.next_flow(start));
        while !end.is_none() {
            let next = basis(s, end, inner, iw);
            if used + gap + next > iw {
                break;
            }
            used += gap + next;
            line_h = line_h.max(s.block_pair(end, inner, ih)[1]);
            end = visible(s, s.next_flow(end));
        }
        place_line(s, l, inner, start, end, pad.x, y, iw, line_h, gap, abs);
        y += line_h + gap;
        start = end;
    }
}

/// Arranges the children from `from` up to but not including `until` on one line.
#[expect(clippy::too_many_arguments, reason = "one line's whole geometry")]
fn place_line(
    s: &mut Solver<'_>,
    l: &Layout,
    inner: WidthClass,
    from: NodeId,
    until: NodeId,
    pad_x: f32,
    y: f32,
    iw: f32,
    line_h: f32,
    gap: f32,
    abs: Vector2,
) {
    let mut used = 0.0f32;
    let mut count = 0usize;
    let mut c = from;
    while !c.is_none() && c != until {
        used += s.geom(c).at_w;
        count += 1;
        c = visible(s, s.next_flow(c));
    }
    used += gap * count.saturating_sub(1) as f32;
    let mut x = pad_x + l.justify.offset(iw, used);
    let mut c = from;
    while !c.is_none() && c != until {
        let cw = s.geom(c).at_w;
        let ch = s.cross_block(c, l.align, inner, line_h);
        let held = s.layout(c);
        let dy = held.align_self_or(l.align).offset(line_h, ch);
        s.arrange(c, Vector2::new(x, y + dy), abs, ch, inner, false);
        x += cw + gap;
        c = visible(s, s.next_flow(c));
    }
}

/// Returns the width a wrapping row breaks lines on.
fn basis(s: &mut Solver<'_>, c: NodeId, class: WidthClass, iw: f32) -> f32 {
    s.inline_pair(c, class, iw)[1]
}

/// Returns `c`, or the first in-flow sibling above it that takes room.
fn visible(s: &Solver<'_>, mut c: NodeId) -> NodeId {
    while !c.is_none() && s.hidden(c) {
        c = s.next_flow(c);
    }
    c
}
