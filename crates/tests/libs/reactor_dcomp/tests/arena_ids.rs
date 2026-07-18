//! `backend/dcomp/node.rs` — the `Arena`'s id contract.
//!
//! **These tests are not headless.** A `Node` owns a compositor
//! `ContainerVisual`, so the arena cannot hold anything without a live
//! `Windows.UI.Composition.Compositor`. The harness stands one up *windowless*
//! — a `DispatcherQueue` on the test thread plus `Compositor::new()`, no HWND,
//! no `DesktopWindowTarget`, no swap chain, nothing on screen — which is the
//! smallest real dependency that makes the arena constructible.
//!
//! If the compositor cannot be created (no interactive session / no
//! coremessaging), these tests **fail loudly** rather than silently passing:
//! a green run must mean the invariant was actually checked.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::{ControlId, ControlKind};

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// Arena-minted ids are monotonic and are NEVER reused.
///
/// The reconciler tracks nodes by id across an unmount/mount sequence
/// (`children_mirror`, the `new_id != old_id` graft check after a component
/// remount). A recycled id aliases a destroyed node: the freshly mounted
/// subtree is never grafted and the destroyed subtree's visuals stay on screen.
/// This is the bug fixed in b90ef0677, when slot indices were recycled.
#[test]
fn arena_ids_are_monotonic_and_never_reused() {
    let mut a = harness();

    let ids: Vec<ControlId> = (0..8)
        .map(|_| a.insert(ControlKind::Border).unwrap())
        .collect();
    let raw: Vec<u32> = ids.iter().map(|i| i.get()).collect();
    assert_eq!(
        raw,
        (1..=8).collect::<Vec<u32>>(),
        "arena ids are not sequential"
    );

    // Free four holes in the middle of the id space.
    for id in &ids[2..6] {
        assert!(a.remove(*id), "remove reported no node for {id}");
    }
    assert_eq!(a.len(), 4);

    // Subsequent mints must skip every hole.
    let mut last = 8u32;
    for _ in 0..16 {
        let id = a.insert(ControlKind::TextBlock).unwrap();
        assert!(
            id.get() > last,
            "{} reused or went backwards from {last}",
            id.get()
        );
        assert!(
            !raw[2..6].contains(&id.get()),
            "{} recycled a freed slot index",
            id.get()
        );
        last = id.get();
    }
}

/// Caller-minted and harness-minted ids share one id space.
///
/// The arena itself no longer mints — the reconciler is the single minter and
/// `insert_with_id` files nodes under ids it chose. The harness mint models
/// that role, and this asserts its watermark contract: minting after a
/// caller-provided id must advance past it, or a later mint collides with a
/// live node — the same aliasing failure, arrived at from the other direction.
#[test]
fn caller_minted_ids_advance_the_arenas_own_counter() {
    let mut a = harness();

    a.insert_with_id(ControlId::new(100), ControlKind::Border)
        .unwrap();
    let next = a.insert(ControlKind::TextBlock).unwrap();
    assert_eq!(
        next.get(),
        101,
        "arena minted {} after a caller-minted 100",
        next.get()
    );
    assert!(
        a.contains(ControlId::new(100)),
        "the caller-minted node vanished"
    );

    // A caller-minted id *below* the watermark must not drag the counter back.
    a.insert_with_id(ControlId::new(5), ControlKind::Button)
        .unwrap();
    let after = a.insert(ControlKind::Border).unwrap();
    assert_eq!(
        after.get(),
        102,
        "a low caller-minted id rewound the counter to {}",
        after.get()
    );
}

/// Interleaving the two paths the way the real seam does — every id from the
/// recorder, then arena-minted ids afterwards — never produces a collision.
#[test]
fn interleaved_minting_paths_never_collide() {
    let mut a = harness();
    let mut seen: Vec<u32> = Vec::new();

    for i in 1..=20u32 {
        let id = ControlId::new(i);
        a.insert_with_id(id, ControlKind::Border).unwrap();
        seen.push(i);
    }
    for _ in 0..20 {
        let id = a.insert(ControlKind::TextBlock).unwrap();
        assert!(
            !seen.contains(&id.get()),
            "arena minted a live id {}",
            id.get()
        );
        seen.push(id.get());
    }
    assert_eq!(a.len(), 40, "a collision silently overwrote a node");
}

/// A node is retrievable by its id and remembers what it was created as —
/// enough to prove `insert_with_id` files the node under the id it was given
/// rather than under a counter value of its own.
#[test]
fn nodes_are_filed_under_the_id_they_were_given() {
    let mut a = harness();
    a.insert_with_id(ControlId::new(42), ControlKind::ToggleSwitch)
        .unwrap();
    assert_eq!(
        a.kind_of(ControlId::new(42)),
        Some(ControlKind::ToggleSwitch)
    );
    assert_eq!(a.kind_of(ControlId::new(41)), None);
    assert_eq!(a.kind_of(ControlId::new(43)), None);
}

/// Removal releases the node (map storage, not slots) and is idempotent.
#[test]
fn remove_releases_the_node_and_is_idempotent() {
    let mut a = harness();
    let id = a.insert(ControlKind::Border).unwrap();
    assert!(a.contains(id));
    assert!(a.remove(id));
    assert!(!a.contains(id));
    assert!(!a.remove(id), "second remove reported a node");
    assert!(a.is_empty());
}
