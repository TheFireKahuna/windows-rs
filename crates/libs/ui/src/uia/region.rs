//! Making a presentation region's contents readable.
//!
//! A region's pixels are a buffer, so nothing inside one can be an entry in the hit array
//! and nothing inside one is a visual. A band handle on the analyzer therefore has no peer
//! at all unless something makes it one — and it is the application's signature gesture.
//!
//! The two halves are owned by different threads and move at different rates, which is the
//! whole reason this module exists rather than a struct:
//!
//! - **geometry** is the renderer's. It republishes whenever its mapping moves — a range
//!   change, a band added, a resize, and every frame of a drag — through
//!   [`RegionParts`], which is versioned so a reader that has seen this version does
//!   nothing at all.
//! - **meaning** is this side's. A part's name and role are declared once and keyed by
//!   [`SubId`]; republishing a name with every geometry change would put an allocation on
//!   a path that has no bound.
//!
//! So `windows-present` names no accessibility type and this crate names no presentation
//! type beyond the two it joins. The join is [`Uia::sync_regions`](super::Uia::sync_regions),
//! called from the tick: one acquire load per watched region when nothing moved.

use super::tree::Part;
use crate::widget::UiaRole;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use windows_present::{RegionParts, SubId};
use windows_scene::ControlId;

/// What one part of a region *is*. Declared once, where the region is.
#[derive(Copy, Clone, Debug)]
pub struct PartDecl {
    pub sub: SubId,
    pub name: &'static str,
    pub role: UiaRole,
}

impl PartDecl {
    #[must_use]
    pub const fn new(sub: u32, name: &'static str, role: UiaRole) -> Self {
        Self {
            sub: SubId(sub),
            name,
            role,
        }
    }
}

/// A region's accessible peer: which control it is, what its parts mean, and where the two
/// live sources are.
pub struct RegionPeer {
    /// The control the region is in the hit array — one entry, like any other.
    pub id: ControlId,
    /// Geometry, as the renderer publishes it.
    pub geometry: Arc<RegionParts>,
    /// One row per part. Order is the order a client reads them in.
    pub parts: Vec<PartDecl>,
    /// One slot per part, indexed by [`SubId`], written by whichever thread owns the
    /// number and read at query time.
    ///
    /// Optional, and separate from the decls for the same reason the geometry is: a band's
    /// gain moves while its name does not. One allocation for the region rather than an
    /// `Arc` per part.
    pub values: Option<Arc<[AtomicU64]>>,
}

/// A watched region, plus what the join needs to not allocate.
pub(super) struct Watched {
    peer: RegionPeer,
    /// The geometry version last joined. The whole of the hot path: a tick where the
    /// renderer has not moved reads this and stops.
    seen: u64,
    /// Both reused, so a drag joins without allocating once the buffers are at their
    /// high-water mark.
    incoming: Vec<windows_present::Part>,
    joined: Vec<Part>,
}

impl Watched {
    pub(super) fn new(peer: RegionPeer) -> Self {
        Self {
            peer,
            // `RegionParts` starts at version zero and a publish bumps it, so a peer whose
            // renderer has already published joins on its first tick rather than never.
            seen: u64::MAX,
            incoming: Vec::new(),
            joined: Vec::new(),
        }
    }

    pub(super) fn id(&self) -> ControlId {
        self.peer.id
    }

    /// Joins geometry against meaning, if the geometry has moved.
    ///
    /// Answers the parts to publish, or `None` when there is nothing to do — which is
    /// every tick but the ones where the renderer's mapping actually changed.
    pub(super) fn join(&mut self) -> Option<&[Part]> {
        let version = self.peer.geometry.version();
        if version == self.seen {
            return None;
        }
        self.seen = self.peer.geometry.read_into(&mut self.incoming);

        self.joined.clear();
        // Driven from the **decls**, not from the geometry: the declared order is the
        // order a client reads the parts in, and a renderer publishing them in whatever
        // order its own mapping produced would otherwise reorder the tree under a reader.
        // A part with no geometry yet is simply not published, rather than published at
        // the origin — which would render as a real element at a real place.
        for decl in &self.peer.parts {
            let Some(found) = self.incoming.iter().find(|part| part.id == decl.sub) else {
                continue;
            };
            self.joined.push(Part {
                sub: decl.sub.0,
                name: decl.name,
                role: decl.role,
                rect: (
                    found.rect.left,
                    found.rect.top,
                    found.rect.right,
                    found.rect.bottom,
                ),
                value: self.value(decl.sub),
            });
        }
        Some(&self.joined)
    }

    fn value(&self, sub: SubId) -> Option<f64> {
        let value = f64::from_bits(
            self.peer
                .values
                .as_ref()?
                .get(sub.0 as usize)?
                .load(Relaxed),
        );
        value.is_finite().then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_present::Rect;

    fn peer(parts: Vec<PartDecl>, values: Option<Arc<[AtomicU64]>>) -> (Arc<RegionParts>, Watched) {
        let geometry = Arc::new(RegionParts::new());
        let mut authority = windows_scene::Ids::<windows_scene::Control>::new();
        let watched = Watched::new(RegionPeer {
            id: authority.mint(),
            geometry: Arc::clone(&geometry),
            parts,
            values,
        });
        (geometry, watched)
    }

    fn decls() -> Vec<PartDecl> {
        vec![
            PartDecl::new(0, "Low band", UiaRole::Slider),
            PartDecl::new(1, "Mid band", UiaRole::Slider),
        ]
    }

    #[test]
    fn a_join_happens_once_per_mapping_change_and_not_once_per_tick() {
        let (geometry, mut watched) = peer(decls(), None);
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

        assert_eq!(watched.join().map(<[Part]>::len), Some(2));
        assert!(
            watched.join().is_none(),
            "a tick where the renderer has not moved does nothing"
        );

        geometry.publish(&[windows_present::Part {
            id: SubId(1),
            rect: Rect::new(40.0, 0.0, 50.0, 100.0),
        }]);
        let joined = watched.join().expect("the mapping moved");
        assert_eq!(joined.len(), 1, "a band that went is not published");
        assert_eq!(joined[0].name, "Mid band");
        assert_eq!(joined[0].rect.0, 40.0);
    }

    #[test]
    fn the_declared_order_is_what_a_client_reads_however_the_renderer_published() {
        let (geometry, mut watched) = peer(decls(), None);
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
        let joined = watched.join().expect("first join");
        assert_eq!(
            joined.iter().map(|part| part.name).collect::<Vec<_>>(),
            ["Low band", "Mid band"],
            "the tree's order is the declared one, not the renderer's"
        );
    }

    #[test]
    fn a_part_reports_the_number_its_own_slot_holds() {
        let values: Arc<[AtomicU64]> = Arc::from([
            AtomicU64::new(f64::NAN.to_bits()),
            AtomicU64::new((-6.5f64).to_bits()),
        ]);
        let (geometry, mut watched) = peer(decls(), Some(Arc::clone(&values)));
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

        let joined = watched.join().expect("first join");
        assert_eq!(joined[0].value, None, "an unwritten slot reports nothing");
        assert_eq!(joined[1].value, Some(-6.5));

        // The producer moves a band; the next mapping change carries the new number.
        values[0].store(3.0f64.to_bits(), Relaxed);
        geometry.publish(&[windows_present::Part {
            id: SubId(0),
            rect: Rect::default(),
        }]);
        assert_eq!(watched.join().expect("moved")[0].value, Some(3.0));
    }

    #[test]
    fn a_peer_whose_renderer_published_before_it_was_watched_still_joins() {
        let geometry = Arc::new(RegionParts::new());
        geometry.publish(&[windows_present::Part {
            id: SubId(0),
            rect: Rect::default(),
        }]);
        let mut authority = windows_scene::Ids::<windows_scene::Control>::new();
        let mut watched = Watched::new(RegionPeer {
            id: authority.mint(),
            geometry,
            parts: decls(),
            values: None,
        });
        assert_eq!(
            watched.join().map(<[Part]>::len),
            Some(1),
            "the first tick joins whatever is already published"
        );
    }
}
