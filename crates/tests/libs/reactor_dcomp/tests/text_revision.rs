//! `backend/dcomp/mod.rs` — the §7.2 arrival rules for revision-stamped
//! editor text (`apply_text_stamped`), front half: composition guard →
//! echo-identical no-op → stale-revision drop → caret-mapped apply.
//!
//! The revision gate is what replaces the old `seeded` focus gate: authority
//! over the buffer is arbitrated by *revision*, not by focus, so a
//! programmatic write can never retract text the user typed after the app
//! was last consulted, and an idle field still accepts a genuine update
//! (preset load, clear-search) without any force lane.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::ControlKind as K;

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// An echo of the exact buffer text is a strict no-op — the caret must not
/// move, which is the difference between a controlled field being usable and
/// the caret teleporting to the end on every render.
#[test]
fn echo_identical_write_never_moves_the_caret() {
    let mut a = harness();
    let id = a.insert(K::TextBox).unwrap();

    a.set_editor_text(id, "hello");
    a.set_editor_caret(id, 2);
    a.apply_text_stamped(id, "hello", 0);

    assert_eq!(a.editor_text(id).as_deref(), Some("hello"));
    assert_eq!(a.editor_caret(id), Some((2, 2)), "the caret moved on an echo");
}

/// A write stamped older than the buffer revision is a stale echo of text the
/// user has superseded: it drops, and the fresh-stamped write that follows
/// (the app converging through the newer intent) applies.
#[test]
fn stale_write_drops_and_fresh_write_applies() {
    let mut a = harness();
    let id = a.insert(K::TextBox).unwrap();

    a.set_editor_text(id, "typed");
    let r1 = a.bump_text_rev(id);
    let r2 = a.bump_text_rev(id);
    assert_eq!((r1, r2), (1, 2), "revisions must be monotonic from 1");

    a.apply_text_stamped(id, "stale echo", r1);
    assert_eq!(
        a.editor_text(id).as_deref(),
        Some("typed"),
        "a write based on rev 1 must drop once the user reached rev 2"
    );

    a.apply_text_stamped(id, "converged", r2);
    assert_eq!(
        a.editor_text(id).as_deref(),
        Some("converged"),
        "a write based on the latest delivered revision applies"
    );
}

/// While an IME composition is active NO programmatic write applies,
/// regardless of stamp — the §7.2 composition guard. When the composition
/// ends, writes flow again.
#[test]
fn composition_guard_blocks_all_writes() {
    let mut a = harness();
    let id = a.insert(K::TextBox).unwrap();

    a.set_editor_text(id, "中");
    a.set_composition_active(id, true);
    a.apply_text_stamped(id, "clobber", u64::MAX);
    assert_eq!(
        a.editor_text(id).as_deref(),
        Some("中"),
        "no write may land inside an active composition"
    );

    a.set_composition_active(id, false);
    a.apply_text_stamped(id, "after", 0);
    assert_eq!(a.editor_text(id).as_deref(), Some("after"));
}

/// A fresh write applies with caret position-mapping — text inserted after
/// the caret leaves it in place (never collapse-to-end).
#[test]
fn fresh_write_applies_with_caret_mapping() {
    let mut a = harness();
    let id = a.insert(K::TextBox).unwrap();

    a.set_editor_text(id, "hello world");
    a.set_editor_caret(id, 5);
    a.apply_text_stamped(id, "hello brave world", 0);

    assert_eq!(a.editor_text(id).as_deref(), Some("hello brave world"));
    assert_eq!(
        a.editor_caret(id),
        Some((5, 5)),
        "a reconciliation write must position-map the caret, not collapse it"
    );
}
