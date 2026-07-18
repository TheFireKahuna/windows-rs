//! `backend/dcomp/caption.rs` — the caption band's derived geometry.
//!
//! `Tall` and `IsBackButtonVisible` used to be *stored*: the value landed in
//! `Extras` and nothing read it, so setting either left the node's layout
//! identical. They are now *consumed* — each re-derives the band's height and
//! its leading padding — and that is exactly what these tests pin: applying
//! one must actually move the node, and applying the value it was born with
//! must leave it indistinguishable from a node that never received the prop.
//!
//! The second half matters more than it looks. The band's left padding reserves
//! the drawn back button, and the back button is born VISIBLE (the shared
//! `Extras` default mirrors `NavigationView`, not `TitleBar`). So the derive
//! and `birth_style` have to agree about a *non-zero* inset on a virgin node.
//! They agree by construction — both call `caption::pad_left` — and if anyone
//! ever re-spells one of them by hand, the reset invariant breaks and these
//! tests say so. The first version of this code did exactly that and did break
//! it.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::{ControlKind as K, Prop, PropValue as V};

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// A TitleBar that has received `prop = value`, digested.
fn with_prop(h: &mut ArenaHarness, prop: Prop, value: V) -> String {
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.apply_prop(id, prop, &value);
    h.node_digest(id).expect("digest")
}

/// A TitleBar that has received nothing.
fn virgin(h: &mut ArenaHarness) -> String {
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.node_digest(id).expect("digest")
}

#[test]
fn tall_changes_the_band_height() {
    let mut h = harness();
    let base = virgin(&mut h);
    let tall = with_prop(&mut h, Prop::Tall, V::Bool(true));
    assert_ne!(
        base, tall,
        "Tall(true) left the node identical — the band height is not being derived"
    );
}

#[test]
fn tall_false_is_the_state_a_titlebar_is_born_in() {
    let mut h = harness();
    let base = virgin(&mut h);
    let explicit = with_prop(&mut h, Prop::Tall, V::Bool(false));
    assert_eq!(
        base, explicit,
        "an explicit Tall(false) differs from a TitleBar that never received it"
    );
}

#[test]
fn tall_round_trips_through_unset() {
    let mut h = harness();
    let base = virgin(&mut h);
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.apply_prop(id, Prop::Tall, &V::Bool(true));
    h.apply_prop(id, Prop::Tall, &V::Unset);
    assert_eq!(
        base,
        h.node_digest(id).expect("digest"),
        "Unsetting Tall did not restore the band height it was born with"
    );
}

#[test]
fn hiding_the_back_button_changes_the_leading_inset() {
    let mut h = harness();
    let base = virgin(&mut h);
    let hidden = with_prop(&mut h, Prop::IsBackButtonVisible, V::Bool(false));
    assert_ne!(
        base, hidden,
        "IsBackButtonVisible(false) left the node identical — the leading \
         inset is not being derived, so a Content child would be laid out \
         under a back button that is not drawn"
    );
}

/// The invariant the derive and `birth_style` share: a virgin TitleBar already
/// reserves the back button (it is born visible), so re-deriving with the
/// default state must reproduce the birth padding EXACTLY — not zero, and not
/// double-counted.
#[test]
fn showing_the_back_button_is_the_state_a_titlebar_is_born_in() {
    let mut h = harness();
    let base = virgin(&mut h);
    let shown = with_prop(&mut h, Prop::IsBackButtonVisible, V::Bool(true));
    assert_eq!(
        base, shown,
        "an explicit IsBackButtonVisible(true) differs from a TitleBar that \
         never received it — birth_style and the caption derive disagree \
         about the leading inset"
    );
}

#[test]
fn back_button_visibility_round_trips_through_unset() {
    let mut h = harness();
    let base = virgin(&mut h);
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.apply_prop(id, Prop::IsBackButtonVisible, &V::Bool(false));
    h.apply_prop(id, Prop::IsBackButtonVisible, &V::Unset);
    assert_eq!(
        base,
        h.node_digest(id).expect("digest"),
        "Unsetting IsBackButtonVisible did not restore the leading inset"
    );
}

/// Re-deriving must be idempotent: the same value applied twice cannot keep
/// adding the inset to itself. (The first implementation read its base from
/// the node's *current* padding instead of its birth padding, and did.)
#[test]
fn re_applying_the_same_caption_state_is_idempotent() {
    let mut h = harness();
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.apply_prop(id, Prop::Tall, &V::Bool(true));
    let once = h.node_digest(id).expect("digest");
    for _ in 0..4 {
        h.apply_prop(id, Prop::Tall, &V::Bool(true));
        h.apply_prop(id, Prop::IsBackButtonVisible, &V::Bool(true));
    }
    // The back button was already visible by default, so the only thing that
    // changed above is that the derive ran four more times.
    assert_eq!(
        once,
        h.node_digest(id).expect("digest"),
        "re-deriving the caption metrics accumulated instead of settling"
    );
}

/// `IsBackButtonEnabled` is a paint-only distinction: the button is drawn
/// either way (greyed when disabled), so the band's geometry must NOT move.
/// If it ever does, a navigation stack hitting depth zero would reflow the
/// whole caption.
#[test]
fn disabling_the_back_button_does_not_move_the_layout() {
    let mut h = harness();
    let id = h.insert(K::TitleBar).expect("insert TitleBar");
    h.apply_prop(id, Prop::IsBackButtonEnabled, &V::Bool(true));
    let enabled = h.node_digest(id).expect("digest");
    h.apply_prop(id, Prop::IsBackButtonEnabled, &V::Bool(false));
    let disabled = h.node_digest(id).expect("digest");
    // The Extras flag differs, so the digests differ — but the *style* half of
    // the digest must not.
    let style_of = |d: &str| {
        d.lines()
            .find(|l| l.starts_with("style="))
            .expect("digest carries a style line")
            .to_string()
    };
    assert_eq!(
        style_of(&enabled),
        style_of(&disabled),
        "enabling/disabling the back button changed the band's layout"
    );
}
