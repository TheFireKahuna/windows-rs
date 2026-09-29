//! Publishes scoped hover identities directly from input to a presentation region.

use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use windows_present::SubId;
use windows_scene::{ControlId, HitTable};
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

    fn publish(&self, selected: Option<SubId>) {
        let value = selected.map_or(NONE, |key| key.0);
        // The self-contained selection is visible before the following epoch wake.
        if self.selected.swap(value, Ordering::Relaxed) != value {
            self.live.epoch.invalidate();
        }
    }
}

#[derive(Clone)]
pub(crate) struct Member {
    pub group: Correlation,
    pub key: Option<SubId>,
}

#[derive(Clone)]
pub(crate) struct Route {
    pub source: ControlId,
    pub member: Member,
}

/// Scene-side declarations forwarded whole only when membership changes.
#[derive(Default)]
pub(crate) struct Routes(Vec<Route>);

impl Routes {
    pub fn adopt(&mut self, changed: &[Route], released: &[ControlId]) {
        self.0.retain(|row| !released.contains(&row.source));
        for row in changed {
            if let Some(old) = self.0.iter_mut().find(|old| old.source == row.source) {
                old.clone_from(row);
            } else {
                self.0.push(row.clone());
            }
        }
    }

    pub fn copy_into(&self, out: &mut Vec<Route>) { out.clone_from(&self.0); }
}

/// Input-side routing storage, refilled only by declaration changes.
#[derive(Default)]
pub(crate) struct Router {
    rows: Vec<Route>,
    hovered: Option<ControlId>,
}

impl Router {
    pub fn sync(&mut self, rows: &[Route], hits: &HitTable) {
        for old in &self.rows {
            if !rows.iter().any(|row| row.member.group.same(&old.member.group)) {
                old.member.group.publish(None);
            }
        }
        self.rows.clear();
        self.rows.extend_from_slice(rows);
        self.refresh(hits);
    }

    pub fn route(&mut self, reports: &[Report], hits: &HitTable) {
        let mut changed = false;
        for report in reports {
            match report {
                Report::HoverChanged { to, .. } => { self.hovered = *to; changed = true; }
                Report::Moved { target, .. } => { self.hovered = Some(*target); changed = true; }
                _ => {}
            }
        }
        if changed { self.refresh(hits); }
    }

    fn refresh(&self, hits: &HitTable) {
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
            group.publish(selected);
        }
    }
}
