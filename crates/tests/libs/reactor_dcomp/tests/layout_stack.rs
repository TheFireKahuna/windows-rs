//! `backend/dcomp/layout.rs` — the composition z-order of a node's own visuals.
//!
//! `layout::sync` rebuilds the child order of any node whose children list or a
//! child's Z-order changed. It used to do that with `Children().RemoveAll()`
//! followed by re-inserting the five categories it knew about — a *total*
//! teardown of a collection it only *partly* owns, so a Knob's arc/needle and an
//! editor's caret (both parented into the node container by their own code)
//! were dropped and never came back.
//!
//! It now detaches only the set it recorded attaching, and lays that set back
//! down beneath everything else. The ordering of that set is a `derive(Ord)` on
//! [`StackKey`]: the band ladder is the z-order and the key's field order is the
//! tie-break chain. These tests sort the shipping types, so they pin the real
//! policy rather than a restatement of it. None of them needs a window.

use windows_reactor::dcomp_test_api::{Band, Slot, StackKey};

fn key(slot: Slot, band: Band, z: i32, doc: usize) -> StackKey {
    StackKey { slot, band, z, doc }
}

/// The band declaration order IS the z-order: chrome under the node's content,
/// then the content, then chrome over it, then the overlay scroll thumb.
/// `restack` lays the sorted stack down bottom → top, so a band that sorts
/// earlier composites lower.
#[test]
fn bands_stack_bottom_to_top_in_declaration_order() {
    let ladder = [
        Band::BelowChrome,
        Band::Content,
        Band::AboveChrome,
        Band::Overlay,
    ];
    for pair in ladder.windows(2) {
        assert!(
            pair[0] < pair[1],
            "{:?} must stack below {:?}",
            pair[0],
            pair[1]
        );
    }
}

/// Content is ordered by `z_index` first, with document position only as the
/// tie-break — a later sibling with a lower z still paints underneath.
#[test]
fn content_sorts_by_z_then_document_order() {
    let mut v = [
        key(Slot::Container, Band::Content, 0, 2),
        key(Slot::Container, Band::Content, 5, 0),
        key(Slot::Container, Band::Content, 0, 1),
        key(Slot::Container, Band::Content, -1, 3),
    ];
    v.sort();
    assert_eq!(
        v.map(|k| (k.z, k.doc)),
        [(-1, 3), (0, 1), (0, 2), (5, 0)],
        "content must sort by (z_index, doc order)"
    );
}

/// A child's `z_index` can never lift it out of its band: a z far above
/// anything the chrome uses still leaves the child under the above-band parts
/// and the scroll thumb, and above the below-band ones. True by construction
/// while the band is the outer key — pinned because flattening the key into a
/// single number would silently break it.
#[test]
fn a_high_z_child_still_sorts_below_the_chrome_above_it() {
    let loud = key(Slot::Container, Band::Content, i32::MAX, 0);
    assert!(loud < key(Slot::Container, Band::AboveChrome, i32::MIN, 0));
    assert!(loud < key(Slot::Container, Band::Overlay, i32::MIN, 0));
    assert!(loud > key(Slot::Container, Band::BelowChrome, i32::MAX, usize::MAX));
}

/// The two collections under a node (its container, and a scroll container's
/// content carrier) are independent stacks, so every container-slot visual
/// sorts before every carrier-slot one regardless of band. `restack` walks the
/// sorted list once in reverse and dispatches per slot; the partition keeps
/// each collection's own subsequence contiguous and in order.
#[test]
fn slots_partition_the_stack() {
    let mut v = [
        key(Slot::Carrier, Band::Content, 0, 0),
        key(Slot::Container, Band::Overlay, 0, 0),
        key(Slot::Carrier, Band::Content, 0, 1),
        key(Slot::Container, Band::BelowChrome, 0, 0),
    ];
    v.sort();
    assert_eq!(
        v.map(|k| k.slot),
        [
            Slot::Container,
            Slot::Container,
            Slot::Carrier,
            Slot::Carrier
        ],
        "the two collections must not interleave"
    );
    assert_eq!(
        v.map(|k| k.doc),
        [0, 0, 0, 1],
        "each slot's own subsequence keeps its order"
    );
}
