//! The generational identity both halves of a control decode.
//!
//! One packing, named once. The app thread unpacks a [`ControlId`](windows_scene::ControlId)
//! to find a control's handlers; the front thread unpacks the same id to find its row; a
//! value row uses the same scheme for the same reason. Three copies of a shift would be three
//! chances for a stale reference to resolve to a live slot instead of to nothing — and the
//! symptom is a queued intent calling into whatever now occupies the index.

/// A dense index and the generation that makes a stale reference a **miss**.
pub(crate) const fn pack(index: u32, generation: u32) -> u64 {
    ((generation as u64) << 32) | index as u64
}

/// The dense index: which slot.
pub(crate) const fn index(id: u64) -> u32 {
    id as u32
}

/// The generation: whether that slot is still the one meant.
pub(crate) const fn generation(id: u64) -> u32 {
    (id >> 32) as u32
}

const _: () = {
    // An index and its generation must survive a round trip.
    let id = pack(7, 3);
    assert!(index(id) == 7 && generation(id) == 3);
};
