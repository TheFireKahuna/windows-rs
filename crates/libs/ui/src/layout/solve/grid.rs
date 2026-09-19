//! The grid arm: track sizing, automatic placement, and the cell a child lands in.
//!
//! Columns are sized before any child is placed — fixed tracks first, then the intrinsic
//! tracks from their single-span items, then what a multi-span item still needs, then the
//! fractional tracks over what is left. Placement is automatic: a child states a cell only
//! where the flow order is not the visual order.

use super::Solver;
use super::flow::{STRIDE, distribute};
use crate::layout::{Align, COLUMN_CAP, Layout, Position, Templates, Track, TrackMax, WidthClass};
use windows_numerics::Vector2;
use windows_scene::NodeId;

/// How many rows automatic placement scans before it gives up and stacks at the last one.
///
/// A grid taller than this is a list, and a list is a virtualized scroll container.
const ROW_CAP: usize = 64;

/// One child's cell, as four scratch words.
const CELL: usize = 4;

/// Answers a grid's summed column widths plus its gaps.
pub(crate) fn measure(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
) -> [f32; 2] {
    let gap = s.gap(l, class, f32::NAN);
    // Walk A's own arm, so every cell's pair is taken here and read by the two walks after.
    let mut c = s.first_flow(n);
    while !c.is_none() {
        s.measure(c, inner);
        c = s.next_flow(c);
    }
    let mut tracks = [Track::AUTO; COLUMN_CAP];
    let cols = columns(s, l, inner, &mut tracks);
    let base = s.scratch.u.len();
    let count = cells(s, n, cols);
    let mut groups = [0.0f32; COLUMN_CAP * STRIDE];
    size_columns(
        s,
        n,
        inner,
        f32::NAN,
        &tracks[..cols],
        base,
        count,
        &mut groups,
    );
    s.scratch.u.truncate(base);
    let gaps = gap * cols.saturating_sub(1) as f32;
    let mut pair = [gaps, gaps];
    for j in 0..cols {
        pair[0] += groups[j * STRIDE];
        pair[1] += groups[j * STRIDE + 1];
    }
    pair
}

/// Gives every cell its width and answers the block pair the rows sum to.
pub(crate) fn place(
    s: &mut Solver<'_>,
    n: NodeId,
    l: &Layout,
    class: WidthClass,
    inner: WidthClass,
    iw: f32,
) -> [f32; 2] {
    let gap = s.gap(l, class, iw);
    let mut tracks = [Track::AUTO; COLUMN_CAP];
    let cols = columns(s, l, inner, &mut tracks);
    let base = s.scratch.u.len();
    let count = cells(s, n, cols);
    let mut groups = [0.0f32; COLUMN_CAP * STRIDE];
    size_columns(s, n, inner, iw, &tracks[..cols], base, count, &mut groups);
    let gaps = gap * cols.saturating_sub(1) as f32;
    distribute(&mut groups[..cols * STRIDE], (iw - gaps).max(0.0));
    let mut heights = [[0.0f32; 2]; ROW_CAP];
    let mut rows = 0usize;
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.place(c, 0.0, inner);
            c = s.next_flow(c);
            continue;
        }
        let cell = read(s, base, k);
        let cw = span_width(&groups, cols, cell[1], cell[3], gap);
        let pair = s.place(c, cw, inner);
        let last = last_row(cell);
        let span = f32::from(cell[2].max(1));
        heights[last] = [heights[last][0].max(pair[0] / span), heights[last][1].max(pair[1] / span)];
        rows = rows.max(last + 1);
        k += 1;
        c = s.next_flow(c);
    }
    s.scratch.u.truncate(base);
    let rows = size_rows(s, l, inner, &mut heights, rows, gap, None);
    let gaps = gap * rows.saturating_sub(1) as f32;
    let (mut least, mut total) = (gaps, gaps);
    for h in heights.iter().take(rows) {
        least += h[0];
        total += h[1];
    }
    [least, total]
}

/// Places every cell against the container's final box.
#[expect(clippy::too_many_arguments, reason = "one container's whole geometry")]
pub(crate) fn arrange(
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
    let mut tracks = [Track::AUTO; COLUMN_CAP];
    let cols = columns(s, l, inner, &mut tracks);
    let base = s.scratch.u.len();
    let count = cells(s, n, cols);
    let mut groups = [0.0f32; COLUMN_CAP * STRIDE];
    size_columns(s, n, inner, iw, &tracks[..cols], base, count, &mut groups);
    let gaps = gap * cols.saturating_sub(1) as f32;
    distribute(&mut groups[..cols * STRIDE], (iw - gaps).max(0.0));
    let mut pairs = [[0.0f32; 2]; ROW_CAP];
    let mut rows = 0usize;
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if !s.hidden(c) {
            let cell = read(s, base, k);
            let last = last_row(cell);
            let (pair, span) = (s.geom(c).pair, f32::from(cell[2].max(1)));
            pairs[last] = [pairs[last][0].max(pair[2] / span), pairs[last][1].max(pair[3] / span)];
            rows = rows.max(last + 1);
            k += 1;
        }
        c = s.next_flow(c);
    }
    let rows = size_rows(s, l, inner, &mut pairs, rows, gap, Some(ih));
    let mut heights = [0.0f32; ROW_CAP];
    for (h, pair) in heights.iter_mut().zip(&pairs).take(rows) {
        *h = pair[1];
    }
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            s.arrange(c, Vector2::zero(), abs, 0.0, inner, true);
            c = s.next_flow(c);
            continue;
        }
        let cell = read(s, base, k);
        let x = pad.x + column_origin(&groups, cell[1] as usize, gap);
        let y = pad.y + row_origin(&heights, cell[0] as usize, gap);
        let room_w = span_width(&groups, cols, cell[1], cell[3], gap);
        let room_h = span_height(&heights, rows, cell[0], cell[2], gap);
        let cw = s.geom(c).at_w;
        let ch = s.cross_block(c, l.align, inner, room_h);
        let held = s.layout(c);
        let dx = held.align_self_or(l.justify).offset(room_w, cw);
        let dy = l.align.offset(room_h, ch);
        s.arrange(c, Vector2::new(x + dx, y + dy), abs, ch, inner, false);
        k += 1;
        c = s.next_flow(c);
    }
    s.scratch.u.truncate(base);
}

// ── tracks ──────────────────────────────────────────────────────────────────────────

/// Fills `out` with this container's column tracks and answers how many there are.
///
/// A container with no template has one column that fills it and is never narrower than its
/// content's minimum.
fn columns(s: &Solver<'_>, l: &Layout, class: WidthClass, out: &mut [Track; COLUMN_CAP]) -> usize {
    let scope = s.scope.at_width(class);
    let held = Templates::with(l.tracks, |t| {
        t.cols.with(scope, |tracks| {
            let n = tracks.len().min(COLUMN_CAP);
            out[..n].copy_from_slice(&tracks[..n]);
            n
        })
    });
    match held {
        Some(n) if n > 0 => n,
        _ => {
            out[0] = Track::min_fr(crate::layout::Len::AUTO, 1.0);
            1
        }
    }
}

/// Fills `groups` with one `[min, nat, fr, max]` per column.
///
/// Fixed tracks take their stated length; intrinsic tracks take the widest of their
/// single-span items; what a multi-span item still needs is added to the intrinsic tracks it
/// spans, in equal shares.
#[expect(clippy::too_many_arguments, reason = "one grid's whole track sizing")]
fn size_columns(
    s: &mut Solver<'_>,
    n: NodeId,
    class: WidthClass,
    iw: f32,
    tracks: &[Track],
    base: usize,
    count: usize,
    groups: &mut [f32; COLUMN_CAP * STRIDE],
) {
    let cols = tracks.len();
    let mut intrinsic = [[0.0f32; 2]; COLUMN_CAP];
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() && k < count {
        if s.hidden(c) {
            c = s.next_flow(c);
            continue;
        }
        let cell = read(s, base, k);
        let pair = s.inline_pair(c, class, iw);
        let col = usize::from(cell[1]).min(cols.saturating_sub(1));
        let span = usize::from(cell[3].max(1));
        if span == 1 {
            intrinsic[col][0] = intrinsic[col][0].max(pair[0]);
            intrinsic[col][1] = intrinsic[col][1].max(pair[1]);
        } else {
            let share = 1.0 / span as f32;
            for j in col..(col + span).min(cols) {
                intrinsic[j][0] = intrinsic[j][0].max(pair[0] * share);
                intrinsic[j][1] = intrinsic[j][1].max(pair[1] * share);
            }
        }
        k += 1;
        c = s.next_flow(c);
    }
    for (j, track) in tracks.iter().enumerate() {
        let least = s.len(track.min, class, iw).unwrap_or(intrinsic[j][0]);
        let (nat, fr, max) = match track.max {
            TrackMax::Len(len) => {
                let v = s.len(len, class, iw).unwrap_or(intrinsic[j][1]);
                (v.max(least), 0.0, v.max(least))
            }
            TrackMax::Fr(weight) => (least, weight.max(0.0), f32::INFINITY),
            TrackMax::MaxContent => (intrinsic[j][1].max(least), 0.0, f32::INFINITY),
            TrackMax::MinContent => (least.max(intrinsic[j][0]), 0.0, f32::INFINITY),
        };
        groups[j * STRIDE] = least;
        groups[j * STRIDE + 1] = nat.max(least);
        groups[j * STRIDE + 2] = fr;
        groups[j * STRIDE + 3] = max.max(least);
    }
}

/// Returns the inline extent of the columns `col` spans, gaps included.
fn span_width(groups: &[f32], cols: usize, col: u16, span: u16, gap: f32) -> f32 {
    let from = usize::from(col).min(cols.saturating_sub(1));
    let to = (from + usize::from(span.max(1))).min(cols);
    let mut w = gap * (to - from).saturating_sub(1) as f32;
    for j in from..to {
        w += groups[j * STRIDE + 1];
    }
    w.max(0.0)
}

/// Returns where column `col` starts, relative to the container's content box.
fn column_origin(groups: &[f32], col: usize, gap: f32) -> f32 {
    let mut x = 0.0;
    for j in 0..col {
        x += groups[j * STRIDE + 1] + gap;
    }
    x
}

fn row_origin(heights: &[f32], row: usize, gap: f32) -> f32 {
    let mut y = 0.0;
    for h in heights.iter().take(row) {
        y += h + gap;
    }
    y
}

fn span_height(heights: &[f32], rows: usize, row: u16, span: u16, gap: f32) -> f32 {
    let from = usize::from(row).min(rows.saturating_sub(1));
    let to = (from + usize::from(span.max(1))).min(rows);
    let mut h = gap * (to - from).saturating_sub(1) as f32;
    for held in heights.iter().take(to).skip(from) {
        h += held;
    }
    h.max(0.0)
}

/// Takes each row's `[minimum, natural]` content height to the pair its track gives it, and
/// answers the row count, which a template may raise past the rows that hold content.
///
/// `room` is the container's inner height, known only when arranging, where the natural slot
/// becomes the row's final height. Measuring passes `None`: a share of the leftover is then
/// worth its floor and a percentage nothing. A row with no track, or a grid with no row
/// template, keeps its content height and shares a stretching grid's leftover evenly.
fn size_rows(
    s: &Solver<'_>,
    l: &Layout,
    class: WidthClass,
    heights: &mut [[f32; 2]; ROW_CAP],
    rows: usize,
    gap: f32,
    room: Option<f32>,
) -> usize {
    let mut tracks = [Track::AUTO; crate::layout::TRACK_CAP];
    let scope = s.scope.at_width(class);
    let stated = Templates::with(l.tracks, |t| {
        t.rows.with(scope, |held| {
            let n = held.len().min(tracks.len());
            tracks[..n].copy_from_slice(&held[..n]);
            n
        })
    })
    .unwrap_or(0);
    let gaps = |rows: usize| gap * rows.saturating_sub(1) as f32;
    if stated == 0 {
        if let Some(room) = room {
            stretch_rows(heights, rows, room - gaps(rows), l);
        }
        return rows;
    }
    let rows = rows.max(stated).min(ROW_CAP);
    let basis = room.unwrap_or(0.0);
    let mut groups = [0.0f32; ROW_CAP * STRIDE];
    for j in 0..rows {
        let (track, [low, content]) = (if j < stated { tracks[j] } else { Track::AUTO }, heights[j]);
        let least = s.len(track.min, class, basis).unwrap_or(low);
        let (nat, fr, max) = match track.max {
            TrackMax::Len(len) => {
                let v = s.len(len, class, basis).unwrap_or(content).max(least);
                (v, 0.0, v)
            }
            TrackMax::Fr(weight) => (least, weight.max(0.0), f32::INFINITY),
            TrackMax::MaxContent | TrackMax::MinContent => (content.max(least), 0.0, f32::INFINITY),
        };
        groups[j * STRIDE..][..STRIDE].copy_from_slice(&[least, nat, fr, max]);
    }
    if let Some(room) = room {
        distribute(&mut groups[..rows * STRIDE], (room - gaps(rows)).max(0.0));
    }
    for j in 0..rows {
        heights[j] = [groups[j * STRIDE], groups[j * STRIDE + 1]];
    }
    rows
}

/// Spreads a stretching grid's leftover block room evenly across its rows.
fn stretch_rows(heights: &mut [[f32; 2]; ROW_CAP], rows: usize, room: f32, l: &Layout) {
    if rows == 0 || l.align != Align::Stretch {
        return;
    }
    let used: f32 = heights.iter().take(rows).map(|h| h[1]).sum();
    if room <= used {
        return;
    }
    let share = (room - used) / rows as f32;
    for h in heights.iter_mut().take(rows) {
        h[1] += share;
    }
}

// ── placement ───────────────────────────────────────────────────────────────────────

/// Assigns every in-flow child a cell and answers how many were assigned.
///
/// A child that states one claims it; the rest are auto-placed row-major into the first run
/// of free slots wide enough for their span.
fn cells(s: &mut Solver<'_>, n: NodeId, cols: usize) -> usize {
    let base = s.scratch.u.len();
    let mut occupied = [0u16; ROW_CAP];
    let mut count = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            c = s.next_flow(c);
            continue;
        }
        s.scratch.u.extend_from_slice(&[0, 0, 1, 1]);
        count += 1;
        c = s.next_flow(c);
    }
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            c = s.next_flow(c);
            continue;
        }
        if let Position::Cell {
            row,
            col,
            row_span,
            col_span,
        } = s.layout(c).position
        {
            let (row, col) = (usize::from(row), usize::from(col).min(cols - 1));
            let rs = usize::from(row_span.max(1));
            let cs = usize::from(col_span.max(1)).min(cols - col);
            claim(&mut occupied, row, col, rs, cs);
            let at = base + k * CELL;
            s.scratch.u[at] = row as u16;
            s.scratch.u[at + 1] = col as u16;
            s.scratch.u[at + 2] = rs as u16;
            s.scratch.u[at + 3] = cs as u16;
        }
        k += 1;
        c = s.next_flow(c);
    }
    let mut cursor = 0usize;
    let mut k = 0usize;
    let mut c = s.first_flow(n);
    while !c.is_none() {
        if s.hidden(c) {
            c = s.next_flow(c);
            continue;
        }
        if s.layout(c).position == Position::Flow {
            let (row, col) = free(&occupied, cols, &mut cursor, 1, 1);
            claim(&mut occupied, row, col, 1, 1);
            let at = base + k * CELL;
            s.scratch.u[at] = row as u16;
            s.scratch.u[at + 1] = col as u16;
        }
        k += 1;
        c = s.next_flow(c);
    }
    count
}

/// Returns the last row a cell reaches.
fn last_row(cell: [u16; 4]) -> usize {
    usize::from(cell[0] + cell[2].max(1) - 1).min(ROW_CAP - 1)
}

/// Returns one child's `[row, col, row_span, col_span]`.
fn read(s: &Solver<'_>, base: usize, k: usize) -> [u16; 4] {
    let at = base + k * CELL;
    [
        s.scratch.u[at],
        s.scratch.u[at + 1],
        s.scratch.u[at + 2],
        s.scratch.u[at + 3],
    ]
}

fn claim(occupied: &mut [u16; ROW_CAP], row: usize, col: usize, rs: usize, cs: usize) {
    let mask = span_mask(col, cs);
    for r in row..(row + rs).min(ROW_CAP) {
        occupied[r] |= mask;
    }
}

fn span_mask(col: usize, cs: usize) -> u16 {
    let mut mask = 0u16;
    for j in col..(col + cs).min(COLUMN_CAP) {
        mask |= 1 << j;
    }
    mask
}

/// Returns the first free cell at or after `cursor`, in row-major order.
fn free(
    occupied: &[u16; ROW_CAP],
    cols: usize,
    cursor: &mut usize,
    rs: usize,
    cs: usize,
) -> (usize, usize) {
    let mut at = *cursor;
    while at < ROW_CAP * cols {
        let (row, col) = (at / cols, at % cols);
        if col + cs <= cols {
            let mask = span_mask(col, cs);
            let taken = (row..(row + rs).min(ROW_CAP)).any(|r| occupied[r] & mask != 0);
            if !taken {
                *cursor = at + 1;
                return (row, col);
            }
        }
        at += 1;
    }
    debug_assert!(false, "a grid auto-places into at most {ROW_CAP} rows");
    (ROW_CAP - 1, 0)
}
