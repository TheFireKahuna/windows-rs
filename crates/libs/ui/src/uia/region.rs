//! Gives the parts of a presentation region an accessible peer.
//!
//! A region's contents are pixels in a buffer, so nothing inside one is an entry in the hit
//! array or a visual of its own. A part becomes readable by joining two sources that
//! different threads own and that move at different rates:
//!
//! - **geometry** is the renderer's, republished through [`RegionParts`] whenever its
//!   mapping moves — a range change, a band added, a resize, every frame of a drag. It is
//!   versioned, so a reader that has already seen a version does no work.
//! - **meaning** is this side's: a part's name and role, declared once as a [`PartDecl`]
//!   and keyed by [`SubId`]. Republishing a name with every geometry change would put an
//!   allocation on a path with no bound.
//!
//! The join is [`Uia::sync_regions`](super::Uia::sync_regions), called from the tick, and
//! costs one acquire load per watched region when nothing moved.

use super::tree::Part;
use crate::widget::UiaRole;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use windows_present::{RegionParts, SubId};
use windows_scene::ControlId;

/// The name and role of one region part, declared once where the region is.
#[derive(Copy, Clone, Debug)]
pub struct PartDecl {
    pub sub: SubId,
    pub name: &'static str,
    pub role: UiaRole,
}

impl PartDecl {
    /// Declares the part the renderer publishes under `sub`.
    #[must_use]
    pub const fn new(sub: u32, name: &'static str, role: UiaRole) -> Self {
        Self {
            sub: SubId(sub),
            name,
            role,
        }
    }
}

/// A region's accessible peer: which control it is, what its parts mean, and where its two
/// live sources are.
pub struct RegionPeer {
    /// The control the region occupies in the hit array, one entry like any other.
    pub id: ControlId,
    /// Geometry, as the renderer publishes it.
    pub geometry: Arc<RegionParts>,
    /// One row per part. Order is the order a client reads them in.
    pub parts: Vec<PartDecl>,
    /// One slot per part, indexed by [`SubId`], written by whichever thread owns the
    /// number and read at query time.
    ///
    /// Optional, and separate from the decls because a band's gain moves while its name
    /// does not. One allocation for the whole region rather than an `Arc` per part.
    pub values: Option<Arc<[AtomicU64]>>,
}

/// A watched region and the buffers its join reuses.
pub(super) struct Watched {
    peer: RegionPeer,
    /// The geometry version last joined. A tick where the renderer has not moved compares
    /// this and stops.
    seen: u64,
    /// Both reused across joins, so a drag allocates nothing once the buffers reach their
    /// high-water mark.
    incoming: Vec<windows_present::Part>,
    joined: Vec<Part>,
}

impl Watched {
    /// Starts watching `peer`, with nothing joined yet.
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

    /// Joins the renderer's geometry against the declared parts, if the geometry has moved.
    ///
    /// Returns the parts to publish, or `None` where the geometry version is unchanged,
    /// which is every tick but the ones where the renderer's mapping moved.
    pub(super) fn join(&mut self) -> Option<&[Part]> {
        let version = self.peer.geometry.version();
        if version == self.seen {
            return None;
        }
        self.seen = self.peer.geometry.read_into(&mut self.incoming);

        self.joined.clear();
        // Driven from the decls, not from the geometry: the declared order is the order a
        // client reads the parts in, and the renderer publishes in whatever order its own
        // mapping produced. A part the geometry does not carry is left out rather than
        // published at the origin, where it would read as a real element at a real place.
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

    /// Returns the number the producer wrote for `sub`, or `None` where the peer declares
    /// no slots, `sub` is past the end, or the slot holds no finite value.
    fn value(&self, sub: SubId) -> Option<f64> {
        // relaxed: the slot stands alone, with no other datum ordered against it, so a
        // reader takes whichever whole value is current.
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
