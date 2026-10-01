use super::*;
use windows_scene::HitTable;

/// Reports a scene-selected insertion in a densely indexed row or grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReorderUpdate {
    pub from: u32,
    pub to: u32,
    /// True only for the first insertion report after the drag threshold.
    pub decided: bool,
}

#[derive(Clone)]
pub(crate) struct ReorderRow {
    pub id: ControlId,
    pub node: NodeId,
    pub group: NodeId,
    pub index: u32,
    pub state: windows_scene::Translation,
}

struct Slot {
    row: ReorderRow,
    rect: [f32; 4],
}

struct Drag {
    epoch: u64,
    slots: Vec<Slot>,
    from: usize,
    to: usize,
    viewport: NodeId,
    scroll_from: Vector2,
    span: [f32; 2],
}

pub(crate) struct Landing {
    group: NodeId,
    index: u32,
    origin: Vector2,
    viewport: NodeId,
    neighbors: Vec<(u32, [f32; 4], windows_scene::TranslationCarry)>,
}

#[derive(Default)]
pub(super) struct Reorders {
    rows: Vec<ReorderRow>,
    held: Option<Drag>,
}

impl Reorders {
    pub fn contains(&self, id: ControlId) -> bool { self.rows.binary_search_by_key(&id, |r| r.id).is_ok() }
    pub fn span(&self) -> Option<[f32; 2]> { self.held.as_ref().map(|drag| drag.span) }

    pub fn adopt(&mut self, rows: &[ReorderRow], released: &[ControlId]) {
        if rows.is_empty() && released.is_empty() { return; }
        self.rows.retain(|r| !released.contains(&r.id));
        for row in rows {
            if let Some(old) = self.rows.iter_mut().find(|r| r.id == row.id) {
                *old = row.clone();
            } else {
                self.rows.push(row.clone());
            }
        }
        if !rows.is_empty() { self.rows.sort_unstable_by_key(|r| r.id); }
    }

    pub fn begin(&mut self, id: ControlId, front: &mut Front<'_>) -> Result<()> {
        self.reset(front)?;
        let Some(source) = self.rows.iter().find(|r| r.id == id) else { return Ok(()) };
        let Some(epoch) = front.scene.drag_preview_epoch() else { return Ok(()) };
        let Some(viewport) = scroll_source(front.scene.hits(), source) else { return Ok(()) };
        let mut slots: Vec<_> = self.rows.iter().filter(|r| r.group == source.group)
            .filter_map(|row| {
                if scroll_source(front.scene.hits(), row) != Some(viewport) { return None; }
                Some(Slot { row: row.clone(), rect: base_rect(front.scene.hits(), row)? })
            })
            .collect();
        slots.sort_unstable_by_key(|s| s.row.index);
        if slots.len() != self.rows.iter().filter(|r| r.group == source.group).count()
            || slots.iter().enumerate().any(|(at, s)| s.row.index as usize != at)
        { return Ok(()); }
        let from = source.index as usize;
        let span = slots.iter().fold([f32::INFINITY, f32::NEG_INFINITY], |span, slot|
            [span[0].min(slot.rect[1]), span[1].max(slot.rect[3])]);
        let scroll_from = front.scene.hits().offset(viewport);
        front.scene.follow_drag_scroll(viewport, front.back);
        front.scene.show_drag_placeholder(0.25, front.back);
        self.held = Some(Drag { epoch, slots, from, to: from, viewport, scroll_from, span });
        Ok(())
    }

    pub fn validate(&mut self, front: &mut Front<'_>) -> Result<bool> {
        if self.held.as_ref().is_some_and(|d| {
            front.scene.drag_preview_epoch() != Some(d.epoch)
                || self.rows.iter().filter(|r| r.group == d.slots[d.from].row.group).count() != d.slots.len()
                || d.slots.iter().any(|s| base_rect(front.scene.hits(), &s.row) != Some(s.rect)
                    || scroll_source(front.scene.hits(), &s.row) != Some(d.viewport))
        }) {
            self.reset(front)?;
            front.scene.end_drag_preview();
            return Ok(true);
        }
        Ok(false)
    }

    pub fn moved(&mut self, at: Point, decided: bool, front: &mut Front<'_>)
        -> Result<(Option<ReorderUpdate>, bool)>
    {
        let reset = self.validate(front)?;
        let Some(drag) = self.held.as_mut() else { return Ok((None, reset)) };
        let scroll = front.scene.hits().offset(drag.viewport);
        let at = Point { x: at.x + scroll.x, y: at.y + scroll.y };
        let to = destination(&drag.slots, drag.from, drag.to, at);
        if to == drag.to && !decided { return Ok((None, false)); }
        drag.to = to;
        let source = drag.slots[drag.from].rect;
        let destination = drag.slots[to].rect;
        front.scene.move_drag_placeholder(Vector2::new(destination[0] - source[0], destination[1] - source[1]));
        let mut changed = false;
        for (index, slot) in drag.slots.iter().enumerate() {
            if index == drag.from { continue; }
            let target = if drag.from < index && index <= to { index - 1 }
                else if to <= index && index < drag.from { index + 1 } else { index };
            let rect = drag.slots[target].rect;
            changed |= displace(&slot.row, Vector2::new(rect[0] - slot.rect[0], rect[1] - slot.rect[1]), front)?;
        }
        Ok((Some(ReorderUpdate { from: drag.from as u32, to: to as u32, decided }), changed))
    }

    pub fn released(&self) -> Option<ReorderUpdate> {
        self.held.as_ref().map(|d| ReorderUpdate { from: d.from as u32, to: d.to as u32, decided: false })
    }

    pub fn landing(&self, epoch: u64, accepted: ControlId, scene: &windows_scene::Scene,
        patch: &windows_scene::SinkPatch) -> Option<Landing>
    {
        let drag = self.held.as_ref().filter(|drag| drag.epoch == epoch)?;
        let source = &drag.slots[drag.from];
        if source.row.id != accepted { return None; }
        let hits = scene.hits();
        let by = hits.translation(accepted) - hits.offset(drag.viewport);
        let neighbors = drag.slots.iter().enumerate().filter_map(|(index, slot)| {
            if index == drag.from { return None; }
            let target = if drag.from < index && index <= drag.to { index - 1 }
                else if drag.to <= index && index < drag.from { index + 1 } else { index };
            Some((target as u32, drag.slots[target].rect, scene.hold_translation(slot.row.node, patch)?))
        }).collect();
        Some(Landing { group: source.row.group, index: drag.to as u32,
            origin: Vector2::new(source.rect[0], source.rect[1]) + by,
            viewport: drag.viewport, neighbors })
    }

    pub fn land(&self, landing: Landing, front: &mut Front<'_>) -> Result<()> {
        for (index, expected, carry) in landing.neighbors {
            let Some(row) = self.rows.iter().find(|row| row.group == landing.group && row.index == index)
                else { continue };
            if base_rect(front.scene.hits(), row) == Some(expected)
                && scroll_source(front.scene.hits(), row) == Some(landing.viewport)
            {
                front.scene.continue_translation(carry, row.node, front.back)?;
            }
        }
        let Some(row) = self.rows.iter().find(|row| row.group == landing.group && row.index == landing.index)
            else { return Ok(()) };
        let Some(rect) = base_rect(front.scene.hits(), row) else { return Ok(()) };
        let Some(viewport) = scroll_source(front.scene.hits(), row) else { return Ok(()) };
        let at = Vector2::new(rect[0], rect[1]) + front.scene.hits().translation(row.id)
            - front.scene.hits().offset(viewport);
        front.scene.land_drag_preview(row.node, landing.origin - at, front.back)
    }

    pub fn follow_preview(&self, by: Vector2, hits: &HitTable) -> bool {
        if !by.x.is_finite() || !by.y.is_finite() { return false; }
        self.held.as_ref().is_some_and(|d|
            d.slots[d.from].row.state.set(by + hits.offset(d.viewport) - d.scroll_from))
    }

    pub fn finish(&mut self, epoch: u64, front: &mut Front<'_>) -> Result<bool> {
        if self.held.as_ref().is_some_and(|d| d.epoch == epoch) { self.reset(front) }
        else { Ok(false) }
    }

    pub fn reset(&mut self, front: &mut Front<'_>) -> Result<bool> {
        let Some(drag) = self.held.take() else { return Ok(false) };
        let mut changed = false;
        for (index, slot) in drag.slots.into_iter().enumerate() {
            changed |= if index == drag.from {
                // The carrier owns the source's visual motion; this word owns its input geometry.
                slot.row.state.set(Vector2::zero())
            } else { displace(&slot.row, Vector2::zero(), front)? };
        }
        Ok(changed)
    }
}

fn displace(row: &ReorderRow, to: Vector2, front: &mut Front<'_>) -> Result<bool> {
    if !row.state.set(to) { return Ok(false); }
    if front.scene.hits().entry(row.id).is_some() {
        front.spring(row.node, Prop::TranslationX, Value::Scalar(to.x))?;
        front.spring(row.node, Prop::TranslationY, Value::Scalar(to.y))?;
    }
    Ok(true)
}

fn base_rect(hits: &HitTable, row: &ReorderRow) -> Option<[f32; 4]> {
    // Drop slots exclude transient translations, including neighbor motion and hover lift.
    let e = hits.entry(row.id)?;
    (e.x1 > e.x0 && e.y1 > e.y0).then_some([e.x0, e.y0, e.x1, e.y1])
}

fn scroll_source(hits: &HitTable, row: &ReorderRow) -> Option<NodeId> {
    let e = hits.entry(row.id)?;
    Some(if e.flags.contains(HitFlags::UNSCROLLED) { NodeId::NONE } else { e.scroll_src })
}

fn destination(slots: &[Slot], from: usize, current: usize, at: Point) -> usize {
    let Some((index, slot)) = slots.iter().enumerate().find(|(_, s)| {
        at.x >= s.rect[0] && at.x <= s.rect[2] && at.y >= s.rect[1] && at.y <= s.rect[3]
    }) else { return current };
    if index == from { return from; }
    let ratio = (at.x - slot.rect[0]) / (slot.rect[2] - slot.rect[0]);
    let insert = if ratio < 0.4 { index } else if ratio > 0.6 { index + 1 } else { return current };
    insert - usize::from(insert > from)
}
