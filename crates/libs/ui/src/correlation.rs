//! Publishes scoped hover identities directly from input to a presentation region.

use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use windows_present::SubId;
use windows_scene::{ControlId, HitTable, NodeId, Prop, Value};
use crate::{input::Report, present::Live};

const NONE: u32 = u32::MAX;

/// Shares one hover selection with the renderer paced by `live`.
#[derive(Clone)]
pub struct Correlation {
    selected: Arc<AtomicU32>,
    live: Live,
}

impl Correlation {
    /// Creates an empty selection using the region's existing presentation epoch.
    pub fn new(live: &Live) -> Self {
        Self { selected: Arc::new(AtomicU32::new(NONE)), live: live.clone() }
    }

    /// Returns the selected member without locking or allocating.
    pub fn selected(&self) -> Option<SubId> {
        // The word carries the complete selection and publishes no associated storage.
        let value = self.selected.load(Ordering::Relaxed);
        (value != NONE).then_some(SubId(value))
    }

    fn same(&self, other: &Self) -> bool { Arc::ptr_eq(&self.selected, &other.selected) }

    fn publish(&self, selected: Option<SubId>) -> bool {
        let value = selected.map_or(NONE, |key| key.0);
        // The self-contained selection is visible before the following epoch wake.
        if self.selected.swap(value, Ordering::Relaxed) != value {
            self.live.epoch.invalidate();
            return true;
        }
        false
    }
}

#[derive(Clone)]
pub(crate) struct Member {
    pub group: Correlation,
    pub key: Option<SubId>,
    pub reveal: NodeId,
}

#[derive(Clone)]
pub(crate) struct Route {
    pub source: ControlId,
    pub member: Member,
}

/// Scene-side declarations forwarded whole only when membership changes.
#[derive(Default)]
pub(crate) struct Routes {
    rows: Vec<Route>,
    lit: Vec<NodeId>,
    next: Vec<NodeId>,
}

impl Routes {
    pub fn adopt(&mut self, changed: &[Route], released: &[ControlId]) {
        self.rows.retain(|row| !released.contains(&row.source));
        for row in changed {
            if let Some(old) = self.rows.iter_mut().find(|old| old.source == row.source) {
                old.clone_from(row);
            } else {
                self.rows.push(row.clone());
            }
        }
        // A reveal target belongs below its declaring scope and retires with it.
        self.lit.retain(|node| self.rows.iter().any(|row| row.member.reveal == *node));
        self.lit.reserve(self.rows.len().saturating_sub(self.lit.len()));
        self.next.reserve(self.rows.len().saturating_sub(self.next.len()));
    }

    pub fn copy_into(&self, out: &mut Vec<Route>) { out.clone_from(&self.rows); }

    pub fn reveal(&mut self, front: &mut crate::widget::Front<'_>) -> windows_core::Result<()> {
        self.next.clear();
        for row in &self.rows {
            let member = &row.member;
            if !member.reveal.is_none() && member.key.is_some()
                && member.key == member.group.selected() {
                self.next.push(member.reveal);
            }
        }
        for (these, others, to) in [(&self.lit, &self.next, 0.0), (&self.next, &self.lit, 1.0)] {
            for &node in these {
                if !others.contains(&node) {
                    front.spring(node, Prop::Opacity, Value::Scalar(to))?;
                }
            }
        }
        core::mem::swap(&mut self.lit, &mut self.next);
        Ok(())
    }
}

/// Input-side routing storage, refilled only by declaration changes.
#[derive(Default)]
pub(crate) struct Router {
    rows: Vec<Route>,
    hovered: Option<ControlId>,
}

impl Router {
    pub fn sync(&mut self, rows: &[Route], hits: &HitTable) -> bool {
        let mut changed = false;
        for old in &self.rows {
            if !rows.iter().any(|row| row.member.group.same(&old.member.group)) {
                changed |= old.member.group.publish(None);
            }
        }
        self.rows.clear();
        self.rows.extend_from_slice(rows);
        changed | self.refresh(hits)
    }

    pub fn route(&mut self, reports: &[Report], hits: &HitTable) -> bool {
        let mut changed = false;
        for report in reports {
            match report {
                Report::HoverChanged { to, .. } => { self.hovered = *to; changed = true; }
                Report::Moved { target, .. } | Report::HoverMoved { target, .. } => {
                    self.hovered = Some(*target); changed = true;
                }
                _ => {}
            }
        }
        changed && self.refresh(hits)
    }

    pub(crate) fn refresh(&self, hits: &HitTable) -> bool {
        let mut changed = false;
        for (at, row) in self.rows.iter().enumerate() {
            let group = &row.member.group;
            if self.rows[..at].iter().any(|old| old.member.group.same(group)) { continue; }
            let mut entry = self.hovered.and_then(|id| hits.entry(id));
            let mut selected = None;
            for _ in 0..hits.entries().len() {
                let Some(current) = entry else { break; };
                if let Some(source) = self.rows.iter().find(|candidate| {
                    candidate.source == current.id && candidate.member.group.same(group)
                }) {
                    selected = source.member.key.or_else(|| group.live.input.hover());
                    break;
                }
                entry = hits.entries().get(current.parent as usize);
            }
            changed |= group.publish(selected);

        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_numerics::Vector2;
    use windows_scene::{CONTROL, HitEntry, HitFlags, Ids, NO_ENTRY};

    fn hits(rows: &[(ControlId, u32)]) -> HitTable {
        let entries: Vec<_> = rows.iter().map(|&(id, parent)| HitEntry {
            id, parent, x0: 0.0, y0: 0.0, x1: 100.0, y1: 100.0,
            clip_parent: NO_ENTRY, scroll_src: NodeId::NONE,
            flags: HitFlags::INTERACTIVE, touch_inflate: 0.0,
        }).collect();
        let mut index: Vec<_> = rows.iter().enumerate().map(|(i, row)| (row.0, i as u32)).collect();
        index.sort_unstable();
        let mut table = HitTable::default();
        table.replace(&entries, &index);
        table
    }

    fn hover(to: Option<ControlId>) -> Report {
        Report::HoverChanged { from: None, to, at: Vector2::new(10.0, 10.0), qpc: 0 }
    }

    #[test]
    fn correlation_routes_descendants_and_region_parts_without_repeated_wakes() {
        let live = Live::new().unwrap();
        let group = Correlation::new(&live);
        let mut ids = Ids::<CONTROL>::default();
        let (card, child, region) = (ids.mint(), ids.mint(), ids.mint());
        let table = hits(&[(card, NO_ENTRY), (child, 0), (region, NO_ENTRY)]);
        let rows = [
            Route { source: card, member: Member { group: group.clone(), key: Some(SubId(9)), reveal: NodeId::NONE } },
            Route { source: region, member: Member { group: group.clone(), key: None, reveal: NodeId::NONE } },
        ];
        let mut router = Router::default();
        assert!(!router.sync(&rows, &table));
        assert!(router.route(&[hover(Some(child))], &table));
        assert_eq!(group.selected(), Some(SubId(9)));
        let seq = live.epoch.seq();
        let allocations = crate::counting::allocations();
        for _ in 0..32 {
            assert!(!router.route(&[hover(Some(card)), hover(Some(child))], &table));
        }
        assert_eq!(crate::counting::allocations(), allocations);
        assert_eq!(live.epoch.seq(), seq);
        live.input.set_hover(Some(SubId(17)));
        assert!(router.route(&[hover(Some(region))], &table));
        assert_eq!(group.selected(), Some(SubId(17)));
        live.input.set_hover(None);
        let at = Vector2::new(15.0, 10.0);
        let sample = crate::input::Sample {
            id: 1, ptype: crate::input::PointerType::Mouse,
            flags: crate::input::PointerFlags(0), at, raw: at,
            contact: (0.0, 0.0), pen: None, time: 0, qpc: 0,
        };
        assert!(router.route(&[Report::Moved { target: region, contact: 1, sample }], &table));
        assert_eq!(group.selected(), None);
        assert!(!router.route(&[hover(None)], &table));
    }

    #[test]
    fn correlation_retirement_clears_a_selection_once_and_preserves_other_groups() {
        let live = Live::new().unwrap();
        let group = Correlation::new(&live);
        let other = Correlation::new(&live);
        let mut ids = Ids::<CONTROL>::default();
        let (card, peer) = (ids.mint(), ids.mint());
        let table = hits(&[(card, NO_ENTRY), (peer, NO_ENTRY)]);
        let rows = [
            Route { source: card, member: Member { group: group.clone(), key: Some(SubId(2)), reveal: NodeId::NONE } },
            Route { source: peer, member: Member { group: other.clone(), key: Some(SubId(3)), reveal: NodeId::NONE } },
        ];
        let mut router = Router::default();
        router.sync(&rows, &table);
        router.route(&[hover(Some(card))], &table);
        assert!(router.sync(&rows[1..], &table));
        assert_eq!(group.selected(), None);
        let seq = live.epoch.seq();
        assert!(!router.sync(&rows[1..], &table));
        assert_eq!(live.epoch.seq(), seq);
        assert!(router.route(&[hover(Some(peer))], &table));
        assert_eq!(other.selected(), Some(SubId(3)));
        assert_eq!(group.selected(), None);
    }
}
