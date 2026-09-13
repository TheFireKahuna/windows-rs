//! Resolving which *part* of a region a contact landed on, and telling the renderer.
//!
//! A region's contents are pixels in a buffer, so nothing inside one is a visual, an entry
//! in the hit array or a node the compositor can animate. Two of the application's central
//! interactions happen inside one anyway — dragging a band handle, and hovering the hero's
//! curve to light the responsible rows — so the path they take is specified rather than
//! improvised.
//!
//! # The region is one hit entry
//!
//! The pointer resolves to the region through the one hit array, like any control, so
//! capture, cancel, the recogniser pool and inertia are unchanged. What this module adds is
//! only *which part* won, resolved after the region's entry did, against the geometry the
//! renderer published.
//!
//! # It goes to the renderer, not to the application
//!
//! The decision is written straight into the region's [`RegionInput`] and its [`Epoch`] is
//! bumped, on this thread. The next present carries the new pixels — one display frame, the
//! same latency the retained path has — and a busy app thread cannot stall a gesture, which
//! is what invariant 4 asks for. The application is told **afterwards**, through the ordinary
//! intent queue, and does the document edit on its own schedule.
//!
//! # What it costs when nothing is happening
//!
//! A tick with no pointer report over a region touches nothing here. A pointer moving over a
//! region whose mapping has not changed reads one atomic — the parts version — and scans a
//! copy this side already holds.

use windows_present::{Part, SubId};
use windows_scene::{ControlId, HitTable};

use crate::input::Report;
use crate::seam::RegionPick;
#[cfg(test)]
use crate::seam::RegionOp;
use crate::widget::{Intent, What};

/// One region the pointer can be picked inside, with the part copy this side scans.
pub(crate) struct PickRow {
    pub pick: RegionPick,
    pub picked: Picked,
}

/// Every region the pointer can be picked inside, as the thread routing a contact holds them.
///
/// Kept beside the table the compositor half holds rather than inside it: the part copy is
/// this side's scratch, and one copy of it is one answer to which part the pointer is on.
#[derive(Default)]
pub(crate) struct Picks {
    rows: Vec<PickRow>,
}

impl Picks {
    /// Applies one batch of region edits, in the order the app half emitted them.
    ///
    /// A region declared with no hit entry is not inserted: nothing can be picked inside a
    /// surface the pointer cannot reach.
    #[cfg(test)]
    pub(crate) fn apply(&mut self, ops: &[RegionOp]) {
        for op in ops {
            match *op {
                RegionOp::Mount {
                    key,
                    control: Some(control),
                    ref live,
                    ..
                } => self.rows.push(PickRow {
                    pick: RegionPick {
                        key,
                        control,
                        live: live.clone(),
                    },
                    picked: Picked::new(),
                }),
                RegionOp::Mount { .. } | RegionOp::Resize { .. } => {}
                RegionOp::Drop { key } => self.rows.retain(|row| row.pick.key != key),
            }
        }
    }

    /// Makes the table hold exactly `picks`: rows whose region is gone are removed, new
    /// regions are added, and a region already held keeps the part copy it has scanned.
    pub(crate) fn sync(&mut self, picks: &[RegionPick]) {
        self.rows
            .retain(|row| picks.iter().any(|pick| pick.key == row.pick.key));
        for pick in picks {
            if !self.rows.iter().any(|row| row.pick.key == pick.key) {
                self.rows.push(PickRow {
                    pick: pick.clone(),
                    picked: Picked::new(),
                });
            }
        }
    }

    /// Returns how many regions the pointer can be picked inside.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns the row the control `id` names, or `None` where that control is not a region.
    ///
    /// A miss is the common case — most reports name an ordinary control — so this is a scan
    /// of a table a settled layout keeps under eight rows long, and it runs only for the
    /// pointer reports [`pick`] acts on.
    fn of(&mut self, id: ControlId) -> Option<&mut PickRow> {
        self.rows.iter_mut().find(|row| row.pick.control == id)
    }
}

/// A region's own copy of the published part geometry, and the version it holds.
///
/// Kept per region rather than rebuilt per pick: a drag over a band publishes a new mapping
/// every frame, and a copy that reached its high-water mark once allocates nothing after
/// that.
#[derive(Default)]
pub(crate) struct Picked {
    /// The version [`parts`](Self::parts) was copied at. [`u64::MAX`] until the first copy,
    /// which is a version a publish counter cannot reach, so a renderer that published
    /// before this side ever looked is copied rather than skipped.
    seen: u64,
    parts: Vec<Part>,
}

impl Picked {
    /// Creates a copy holding nothing, at a version no publish can have produced.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            seen: u64::MAX,
            parts: Vec::new(),
        }
    }

    /// Returns the part at `local`, refreshing the copy first if the renderer republished.
    ///
    /// `local` is in the region's own DIPs, which is the space the renderer publishes in.
    ///
    /// **Last match wins.** The renderer publishes in whatever order its mapping produced,
    /// and a later part is drawn over an earlier one, so the topmost is the last that
    /// contains the point — the same rule the hit array resolves overlap by.
    fn at(&mut self, geometry: &windows_present::RegionParts, local: (f32, f32)) -> Option<SubId> {
        let version = geometry.version();
        if version != self.seen {
            self.seen = geometry.read_into(&mut self.parts);
        }
        self.parts
            .iter()
            .rev()
            .find(|part| {
                let r = part.rect;
                local.0 >= r.left && local.0 <= r.right && local.1 >= r.top && local.1 <= r.bottom
            })
            .map(|part| part.id)
    }
}

/// Applies this tick's pointer reports to whichever regions they landed on.
///
/// Called from the tick after the front table has moved its pixels, so a region's own
/// publish sits beside every other control's rather than ahead of it.
///
/// `out` receives one intent per part a gesture finished on, which is how the application
/// learns what was edited. Nothing is queued for a hover: a hover changes pixels and no
/// document, and an intent per pointer sample would put the app thread on the frame clock.
pub(crate) fn pick(reports: &[Report], hits: &HitTable, picks: &mut Picks, out: &mut Vec<Intent>) {
    for report in reports {
        match *report {
            // Both edges, in one pass. Leaving the region clears its hover, and a fast flick
            // across two regions publishes the leave before the enter, because the reports
            // arrive in the order the pointer crossed them.
            Report::HoverChanged { from, to, at, .. } => {
                if let Some(from) = from {
                    clear_hover(picks, from);
                }
                if let Some(to) = to {
                    hover(picks, to, (at.x, at.y), hits);
                }
            }
            Report::Moved {
                target, ref sample, ..
            } => hover(picks, target, (sample.raw.x, sample.raw.y), hits),
            Report::Pressed {
                target, ref sample, ..
            } => press(picks, target, (sample.raw.x, sample.raw.y), hits),
            // A release commits: the part under the contact stops being active, and the
            // application is told which part the gesture finished on.
            Report::Released { target, at, .. } => {
                if let Some(sub) = release(picks, target, (at.x, at.y), hits) {
                    out.push(Intent {
                        target,
                        what: What::Committed(f64::from(sub.0)),
                    });
                }
            }
            // A cancel restores and commits nothing, so it clears the active part and queues
            // no intent — the same contract a slider's canceled drag has.
            Report::Canceled { target, .. } => {
                if let Some(row) = picks.of(target) {
                    row.pick.live.input.set_active(None);
                    row.pick.live.epoch.bump();
                }
            }
            _ => {}
        }
    }
}

/// Publishes the hovered part and the cursor for the region `id` names.
fn hover(picks: &mut Picks, id: ControlId, at: (f32, f32), hits: &HitTable) {
    let Some(local) = local_of(id, at, hits) else {
        return;
    };
    let Some(row) = picks.of(id) else { return };
    let sub = row.picked.at(&row.pick.live.parts, local);
    row.pick.live.input.set_hover(sub);
    row.pick.live.input.set_cursor(Some(local));
    row.pick.live.epoch.bump();
}

/// Clears the hover and the cursor for the region `id` names.
///
/// The cursor goes with the hover: a readout drawn at the last position the pointer held
/// while it is somewhere else entirely states a measurement that is not being taken.
fn clear_hover(picks: &mut Picks, id: ControlId) {
    if let Some(row) = picks.of(id) {
        row.pick.live.input.set_hover(None);
        row.pick.live.input.set_cursor(None);
        row.pick.live.epoch.bump();
    }
}

/// Publishes the part an in-flight gesture is on.
fn press(picks: &mut Picks, id: ControlId, at: (f32, f32), hits: &HitTable) {
    let Some(local) = local_of(id, at, hits) else {
        return;
    };
    let Some(row) = picks.of(id) else { return };
    let sub = row.picked.at(&row.pick.live.parts, local);
    row.pick.live.input.set_active(sub);
    row.pick.live.input.set_cursor(Some(local));
    row.pick.live.epoch.bump();
}

/// Clears the active part and returns the one the gesture finished on.
fn release(picks: &mut Picks, id: ControlId, at: (f32, f32), hits: &HitTable) -> Option<SubId> {
    let local = local_of(id, at, hits)?;
    let row = picks.of(id)?;
    let sub = row.picked.at(&row.pick.live.parts, local);
    row.pick.live.input.set_active(None);
    row.pick.live.epoch.bump();
    sub
}

/// Converts a client-DIP point into the region's own space.
///
/// The origin is the region's hit entry, which is where layout put it in the same
/// unscrolled, pixel-snapped space the array is scanned in — so a region inside a scrolled
/// container resolves against the box the contact was already matched to, and nothing here
/// re-derives a scroll offset.
///
/// `None` where the entry has gone: a report can outlive one tick's array by a frame, and a
/// point resolved against a missing box would land at the window's origin rather than
/// nowhere.
fn local_of(id: ControlId, at: (f32, f32), hits: &HitTable) -> Option<(f32, f32)> {
    let entry = hits.entry(id)?;
    Some((at.0 - entry.x0, at.1 - entry.y0))
}
