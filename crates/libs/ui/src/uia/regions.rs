//! What a presentation region declares about itself, held beside the snapshot.
//!
//! A region's contents are pixels in a buffer, so nothing inside one is an entry or a visual
//! of its own. A part becomes readable by joining two sources that different threads own and
//! that move at different rates:
//!
//! - **geometry** is the renderer's, republished through [`RegionParts`] whenever its mapping
//!   moves — a range change, a band added, a resize, every frame of a drag. It is versioned,
//!   so a reader that has already seen a version does no work.
//! - **meaning** is this side's: a part's name and role, declared once as a [`PartDecl`] and
//!   keyed by [`SubId`]. Republishing a name with every geometry change would put an
//!   allocation on a path with no bound.
//!
//! A part's **number** is neither: it is one `AtomicU64` slot per part written by whichever
//! thread owns it, read at query time, so a band's gain can move while its name and its
//! rectangle do not.

use super::snapshot::Part;
use crate::widget::UiaRole;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use windows_present::{RegionParts, SubId};
use windows_scene::{ControlId, Point};

/// The sub id standing for the region itself rather than one of its parts.
pub const NO_PART: u32 = u32::MAX;

/// Encodes an unavailable formatted reading; measured NaNs use the canonical NaN instead.
pub const MISSING_READING: u64 = 0x7ff8_0000_0000_0001;

/// Coalesces requested notifications for up to 64 authoritative region readings.
#[derive(Default)]
pub struct PartUpdates {
    pub(super) shared: Mutex<Weak<super::provider::Shared>>,
    dirty: AtomicU64,
}
impl PartUpdates {
    /// Records changed sub-IDs as bits; decorative frame changes must not call this.
    pub fn changed(&self, mask: u64) {
        if mask == 0 { return; }
        let Some(shared) = self.shared.lock().unwrap_or_else(PoisonError::into_inner).upgrade() else { return; };
        if !shared.advised.subscribed(crate::bindings::UIA_AutomationPropertyChangedEventId) { return; }
        // Release: the UIA thread sees the values stored before their notification bits.
        if self.dirty.fetch_or(mask, std::sync::atomic::Ordering::Release) == 0 { shared.wake(); }
    }
}

/// The name and role of one region part, declared once where the region is.
#[derive(Copy, Clone, Debug)]
pub struct PartDecl {
    pub sub: SubId,
    pub name: &'static str,
    pub role: UiaRole,
    pub format: Option<fn(Option<f64>) -> String>,
}

impl PartDecl {
    /// Declares the part the renderer publishes under `sub`.
    #[must_use]
    pub const fn new(sub: u32, name: &'static str, role: UiaRole) -> Self {
        Self {
            sub: SubId(sub),
            name,
            role,
            format: None,
        }
    }
    /// Formats a producer reading on demand for a read-only value query.
    #[must_use]
    pub const fn formatted(mut self, format: fn(Option<f64>) -> String) -> Self {
        self.format = Some(format);
        self
    }

}

/// A region's accessible peer: which control it is, what its parts mean, and where its two
/// live sources are.
pub struct RegionPeer {
    /// Formats the region's own producer value when automation queries it.
    pub format: Option<fn(Option<f64>) -> String>,
    /// The control the region occupies in the hit array, one entry like any other.
    pub id: ControlId,
    /// Geometry, as the renderer publishes it.
    pub geometry: Arc<RegionParts>,
    /// One row per part. Order is the order a client reads them in.
    pub parts: Vec<PartDecl>,
    /// One slot per part, indexed by [`SubId`], written by whichever thread owns the number.
    /// One allocation for the whole region rather than an `Arc` per part.
    pub values: Option<Arc<[AtomicU64]>>,
    pub updates: Option<Arc<PartUpdates>>,
    /// The slot the region itself reports its number from.
    pub value: Option<Arc<AtomicU64>>,
}

/// One declared region and the buffers its join reuses.
struct Row {
    format: Option<fn(Option<f64>) -> String>,
    updates: Option<Arc<PartUpdates>>,
    announced: Vec<u64>,
    id: ControlId,
    decls: Vec<PartDecl>,
    geometry: Option<Arc<RegionParts>>,
    /// The region's own number.
    value: Option<Arc<AtomicU64>>,
    /// Its parts' numbers, indexed by [`SubId`].
    values: Option<Arc<[AtomicU64]>>,
    /// The geometry version last joined. A tick where the renderer has not moved compares
    /// this and stops.
    seen: u64,
    /// Both reused across joins, so a drag allocates nothing once they reach their high-water
    /// mark.
    incoming: Vec<windows_present::Part>,
    joined: Vec<Part>,
}

impl Row {
    fn new(id: ControlId) -> Self {
        Self {
            format: None,
            updates: None,
            announced: Vec::new(),
            id,
            decls: Vec::new(),
            geometry: None,
            value: None,
            values: None,
            // `RegionParts` starts at version zero and a publish bumps it, so a peer whose
            // renderer has already published joins on its first tick rather than never.
            seen: u64::MAX,
            incoming: Vec::new(),
            joined: Vec::new(),
        }
    }

    /// Rebuilds the published parts if the renderer's mapping moved, and returns whether it did.
    ///
    /// Driven from the decls, not from the geometry: the declared order is the order a client
    /// reads the parts in, and the renderer publishes in whatever order its own mapping
    /// produced. A part the geometry does not carry is left out rather than published at the
    /// origin, where it would read as a real element at a real place.
    fn join(&mut self) -> bool {
        let Some(geometry) = self.geometry.as_ref() else {
            return false;
        };
        if geometry.version() == self.seen {
            return false;
        }
        self.seen = geometry.read_into(&mut self.incoming);
        self.joined.clear();
        for decl in &self.decls {
            let Some(found) = self.incoming.iter().find(|part| part.id == decl.sub) else {
                continue;
            };
            self.joined.push(Part {
                sub: decl.sub.0,
                name: decl.name,
                role: decl.role,
                rect: [
                    found.rect.left,
                    found.rect.top,
                    found.rect.right,
                    found.rect.bottom,
                ],
            });
        }
        true
    }
}

/// Every region's declaration, behind one lock.
///
/// The lock is held for the length of one read of one row and is taken by the front thread
/// once per moved mapping, so a query never waits on more than a slice walk.
///
/// # Contract
///
/// A closure handed to [`with_subs`](Regions::with_subs) must not call back into this table,
/// which would deadlock on the same lock.
#[derive(Default)]
pub struct Regions(Mutex<Vec<Row>>);

impl Regions {
    /// Runs `edit` on `id`'s row, adding it on first use.
    fn row<R>(&self, id: ControlId, edit: impl FnOnce(&mut Row) -> R) -> R {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let at = held.iter().position(|row| row.id == id).unwrap_or_else(|| {
            held.push(Row::new(id));
            held.len() - 1
        });
        edit(&mut held[at])
    }

    /// Runs `read` on `id`'s row, or on nothing where it has declared none.
    fn get<R>(&self, id: ControlId, read: impl FnOnce(&Row) -> Option<R>) -> Option<R> {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        read(held.iter().find(|row| row.id == id)?)
    }

    /// Replaces the parts `id` publishes, for a region whose owner states them directly rather
    /// than through a watched renderer.
    pub fn set_parts(&self, id: ControlId, parts: &[Part]) {
        self.row(id, |row| {
            row.joined.clear();
            row.joined.extend_from_slice(parts);
        });
    }

    /// Binds the producer-owned cell holding the region's own number.
    pub fn bind_value(&self, id: ControlId, cell: Arc<AtomicU64>) {
        self.row(id, |row| row.value = Some(cell));
    }

    /// Reports whether a producer owns the region's numeric value, including absence.
    pub fn has_value(&self, id: ControlId) -> bool {
        self.get(id, |row| Some(row.value.is_some())).unwrap_or(false)
    }

    /// Starts watching `peer`, replacing any earlier declaration on the same control.
    pub fn watch(&self, peer: RegionPeer) {
        self.row(peer.id, |row| {
            row.format = peer.format;
            if let Some(previous) = &row.updates {
                if peer.updates.as_ref().is_none_or(|next| !Arc::ptr_eq(previous,next)) {
                    *previous.shared.lock().unwrap_or_else(PoisonError::into_inner) = Weak::new();
                }
            }
            row.decls = peer.parts;
            row.geometry = Some(peer.geometry);
            row.values = peer.values;
            row.updates = peer.updates;
            row.announced.clear();
            if let Some(values) = &row.values { row.announced.extend(values.iter().map(|v| v.load(Relaxed))); }
            // A cell bound separately stands: a peer that declares none is stating its parts,
            // not withdrawing the region's own number.
            if peer.value.is_some() {
                row.value = peer.value;
            }
            row.seen = u64::MAX;
        });
    }

    pub(super) fn notifications(&self, pending: &mut super::events::Pending) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for row in held.iter_mut() {
            let Some(updates) = &row.updates else { continue; };
            // Acquire: pairs with the producer's release before announcing its value.
            let mask = updates.dirty.swap(0, std::sync::atomic::Ordering::Acquire);
            if mask == 0 { continue; }
            let Some(values) = &row.values else { continue; };
            for decl in &row.decls {
                let index = decl.sub.0 as usize;
                if index >= 64 || mask & (1 << index) == 0 { continue; }
                let (Some(value), Some(old), Some(format)) = (values.get(index), row.announced.get_mut(index), decl.format) else { continue; };
                let now = value.load(Relaxed);
                if now == *old { continue; }
                let before = format((*old != MISSING_READING).then(|| f64::from_bits(*old)));
                *old = now;
                pending.push(super::events::Raise::PartValue(row.id,decl.sub.0,super::events::Val::Text(before.encode_utf16().collect())));
            }
        }
    }

    /// Drops what `id` declares, so nothing is published for it.
    pub fn forget(&self, id: ControlId) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|row| {
                if row.id != id { return true; }
                if let Some(updates) = &row.updates {
                    *updates.shared.lock().unwrap_or_else(PoisonError::into_inner) = Weak::new();
                }
                false
            });
    }

    /// Re-joins every watched region whose renderer has moved, and returns whether any did.
    ///
    /// A region whose geometry version has not moved costs one acquire load and nothing else,
    /// so this can sit on the tick unconditionally.
    pub fn sync(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter_mut()
            .fold(false, |moved, row| row.join() || moved)
    }

    /// Collects every region that publishes a number of its own, with the number it holds.
    ///
    /// Reuses `out`'s allocation, so a tick that finds the same regions allocates nothing.
    pub fn readings(&self, out: &mut Vec<(ControlId, f64)>) {
        out.clear();
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for row in held.iter() {
            let Some(cell) = row.value.as_ref() else {
                continue;
            };
            let value = f64::from_bits(cell.load(Relaxed));
            if value.is_finite() {
                out.push((row.id, value));
            }
        }
    }

    /// Returns the part `sub` names under `id`.
    pub fn part(&self, id: ControlId, sub: u32) -> Option<Part> {
        self.get(id, |row| {
            row.joined.iter().find(|part| part.sub == sub).copied()
        })
    }

    /// Runs `read` with the parts `id` publishes, in declared order.
    pub fn with_subs<R>(&self, id: ControlId, read: impl FnOnce(&[Part]) -> R) -> R {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match held.iter().find(|row| row.id == id) {
            Some(row) => read(&row.joined),
            None => read(&[]),
        }
    }

    /// Returns the part covering a region-local point, or [`NO_PART`].
    pub fn pick(&self, id: ControlId, point: Point) -> u32 {
        self.with_subs(id, |parts| {
            parts
                .iter()
                .find(|part| {
                    point.x >= part.rect[0]
                        && point.x <= part.rect[2]
                        && point.y >= part.rect[1]
                        && point.y <= part.rect[3]
                })
                .map_or(NO_PART, |part| part.sub)
        })
    }

    /// Reports whether a region or part declares a read-only formatted value.
    pub fn has_formatted_value(&self, id: ControlId, sub: u32) -> bool {
        self.get(id, |row| Some(if sub == NO_PART {
            row.format.is_some()
        } else {
            row.decls.iter().any(|d| d.sub.0 == sub && d.format.is_some())
        })).unwrap_or(false)
    }

    /// Formats a region or part's authoritative value without rebuilding the scene snapshot.
    pub fn formatted_value(&self, id: ControlId, sub: u32) -> Option<String> {
        let (format, bits) = self.get(id, |row| {
            if sub == NO_PART {
                return Some((row.format?, row.value.as_ref()?.load(Relaxed)));
            }
            let format = row.decls.iter().find(|d| d.sub.0 == sub)?.format?;
            let bits = row.values.as_ref()?.get(sub as usize)?.load(Relaxed);
            Some((format, bits))
        })?;
        Some(format((bits != MISSING_READING).then(|| f64::from_bits(bits))))
    }

    /// Returns the number a producer wrote for `id`, or for one of its parts.
    ///
    /// [`NO_PART`] reads the region's own cell. Either way the slot is read here rather than
    /// captured when the geometry last moved, so a band whose gain changes without its
    /// rectangle changing reports the number it holds now.
    ///
    /// Relaxed: each slot stands alone, with no other datum ordered against it, so a reader
    /// takes whichever whole value is current.
    pub fn value(&self, id: ControlId, sub: u32) -> Option<f64> {
        self.get(id, |row| {
            let bits = if sub == NO_PART {
                row.value.as_ref()?.load(Relaxed)
            } else {
                row.values.as_ref()?.get(sub as usize)?.load(Relaxed)
            };
            let value = f64::from_bits(bits);
            value.is_finite().then_some(value)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_present::Rect;

    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<{ windows_scene::CONTROL }>::default();
        (0..count).map(|_| authority.mint()).collect()
    }

    fn decls() -> Vec<PartDecl> {
        vec![
            PartDecl::new(0, "Low band", UiaRole::Slider),
            PartDecl::new(1, "Mid band", UiaRole::Slider),
        ]
    }

    /// Returns a table watching one region, that region's control id, and its geometry.
    fn watched(values: Option<Arc<[AtomicU64]>>) -> (Regions, ControlId, Arc<RegionParts>) {
        let id = ids(1)[0];
        let geometry = Arc::new(RegionParts::new());
        let regions = Regions::default();
        regions.watch(RegionPeer {
            format: None,
            updates: None,
            id,
            geometry: Arc::clone(&geometry),
            parts: decls(),
            values,
            value: None,
        });
        (regions, id, geometry)
    }

    fn count(regions: &Regions, id: ControlId) -> usize {
        regions.with_subs(id, <[Part]>::len)
    }

    #[test]
    fn whole_region_text_distinguishes_absence_silence_and_nonfinite_without_a_join() {
        let id = ids(1)[0];
        let value = Arc::new(AtomicU64::new(MISSING_READING));
        let regions = Regions::default();
        regions.watch(RegionPeer {
            id,
            geometry: Arc::new(RegionParts::new()),
            parts: Vec::new(),
            values: None,
            updates: None,
            value: Some(value.clone()),
            format: Some(|value| match value {
                None => "Unavailable".into(),
                Some(f64::NEG_INFINITY) => "Silence".into(),
                Some(value) if !value.is_finite() => "Nonfinite".into(),
                Some(value) => format!("{value:.1} dBFS"),
            }),
        });
        assert!(regions.has_formatted_value(id, NO_PART));
        for (bits, text, number) in [
            (MISSING_READING, "Unavailable", None),
            (f64::NEG_INFINITY.to_bits(), "Silence", None),
            (f64::NAN.to_bits(), "Nonfinite", None),
            ((-12.04f64).to_bits(), "-12.0 dBFS", Some(-12.04)),
        ] {
            value.store(bits, Relaxed);
            assert_eq!(regions.formatted_value(id, NO_PART).as_deref(), Some(text));
            assert_eq!(regions.value(id, NO_PART), number);
            assert_eq!(count(&regions, id), 0);
        }
        regions.forget(id);
        assert!(!regions.has_formatted_value(id, NO_PART));
        assert_eq!(regions.formatted_value(id, NO_PART), None);
    }

    #[test]
    fn a_join_happens_once_per_mapping_change_and_not_once_per_tick() {
        let (regions, id, geometry) = watched(None);
        geometry.publish(&[
            windows_present::Part {
                id: SubId(0),
                rect: Rect::new(0.0, 0.0, 10.0, 100.0),
            },
            windows_present::Part {
                id: SubId(1),
                rect: Rect::new(20.0, 0.0, 30.0, 100.0),
            },
        ]);
        assert!(regions.sync(), "the first tick joins");
        assert_eq!(count(&regions, id), 2);
        assert!(
            !regions.sync(),
            "a tick where the renderer has not moved does nothing"
        );

        geometry.publish(&[windows_present::Part {
            id: SubId(1),
            rect: Rect::new(40.0, 0.0, 50.0, 100.0),
        }]);
        assert!(regions.sync(), "the mapping moved");
        assert_eq!(count(&regions, id), 1, "a band that went is not published");
        assert_eq!(regions.part(id, 1).expect("still published").rect[0], 40.0);
    }

    #[test]
    fn the_declared_order_is_what_a_client_reads_however_the_renderer_published() {
        let (regions, id, geometry) = watched(None);
        geometry.publish(&[
            windows_present::Part {
                id: SubId(1),
                rect: Rect::new(20.0, 0.0, 30.0, 100.0),
            },
            windows_present::Part {
                id: SubId(0),
                rect: Rect::new(0.0, 0.0, 10.0, 100.0),
            },
        ]);
        regions.sync();
        let names = regions.with_subs(id, |parts| {
            parts.iter().map(|part| part.name).collect::<Vec<_>>()
        });
        assert_eq!(
            names,
            ["Low band", "Mid band"],
            "the published order is the declared one, not the renderer's"
        );
    }

    /// The number is read where it is asked for, so a band whose gain moves without its
    /// rectangle moving reports what the slot holds now.
    #[test]
    fn a_part_reports_the_number_its_own_slot_holds() {
        let values: Arc<[AtomicU64]> = Arc::from([
            AtomicU64::new(f64::NAN.to_bits()),
            AtomicU64::new((-6.5f64).to_bits()),
        ]);
        let (regions, id, geometry) = watched(Some(Arc::clone(&values)));
        geometry.publish(&[
            windows_present::Part {
                id: SubId(0),
                rect: Rect::default(),
            },
            windows_present::Part {
                id: SubId(1),
                rect: Rect::default(),
            },
        ]);
        regions.sync();

        assert_eq!(
            regions.value(id, 0),
            None,
            "an unwritten slot reports nothing"
        );
        assert_eq!(regions.value(id, 1), Some(-6.5));

        values[0].store(3.0f64.to_bits(), Relaxed);
        assert_eq!(
            regions.value(id, 0),
            Some(3.0),
            "and the number is current without the geometry having moved"
        );
    }

    #[test]
    fn a_peer_whose_renderer_published_before_it_was_watched_still_joins() {
        let id = ids(1)[0];
        let geometry = Arc::new(RegionParts::new());
        geometry.publish(&[windows_present::Part {
            id: SubId(0),
            rect: Rect::default(),
        }]);
        let regions = Regions::default();
        regions.watch(RegionPeer {
            format: None,
            updates: None,
            id,
            geometry,
            parts: decls(),
            values: None,
            value: None,
        });
        assert!(
            regions.sync(),
            "the first tick joins what is already published"
        );
        assert_eq!(count(&regions, id), 1);
    }

    #[test]
    fn a_released_control_leaves_nothing_behind() {
        let (regions, id, geometry) = watched(None);
        geometry.publish(&[windows_present::Part {
            id: SubId(0),
            rect: Rect::default(),
        }]);
        regions.sync();
        assert_eq!(count(&regions, id), 1);
        regions.forget(id);
        assert_eq!(count(&regions, id), 0);
        assert!(!regions.sync(), "and nothing is left to join");
    }
}
