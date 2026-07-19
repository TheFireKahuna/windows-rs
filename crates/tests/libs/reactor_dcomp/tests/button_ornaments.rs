//! `backend/dcomp/controls.rs` — a button's content geometry, and the promise
//! that the family never draws.
//!
//! `button_boxes` is one definition with four consumers: the label's glyph
//! sprites, the icon's, the badge plate's `parts::button_sync`, and the layout
//! measure that reserves the width all three need. They agree only because they
//! ask the same function — so what is worth pinning here is not any single
//! box's numbers but the *relationships* between them: that ornaments never
//! overlap the label, that the label keeps whatever is left, and that the
//! measure reserves exactly what the boxes consume.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::{Badge, Color, ControlKind as K, Prop, PropValue as V};

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// A button of a usable size carrying `text`, plus whatever props the case adds.
fn button(a: &mut ArenaHarness, text: &str) -> windows_reactor::ControlId {
    let id = a.insert(K::Button).unwrap();
    a.apply_prop(id, Prop::Content, &V::Str(text.into()));
    a.set_rect(id, 200.0, 32.0);
    id
}

const ICON: i32 = 0xE72C; // Segoe Fluent "Refresh"

// ── The family draws nothing ─────────────────────────────────────────────────

/// The whole point of the retained rewrite: no member of the family is given a
/// surface, whatever it is carrying. A regression here is silent — the button
/// keeps looking right while quietly allocating a surface and entering
/// `BeginDraw` on every state change.
#[test]
fn no_button_in_the_family_ever_wants_a_surface() {
    let mut a = harness();
    for kind in [K::Button, K::ToggleButton, K::RepeatButton, K::SplitButton] {
        let id = a.insert(kind).unwrap();
        a.apply_prop(id, Prop::Content, &V::Str("Apply".into()));
        a.set_rect(id, 200.0, 32.0);
        assert_eq!(a.has_chrome(id), Some(false), "{kind:?} bare");

        // The three things that used to force a draw: an icon, a badge, and
        // focus (which the shared painted focus ring needed a surface for).
        a.apply_prop(id, Prop::Icon, &V::I32(ICON));
        a.apply_prop(id, Prop::Badge, &V::Badge(Badge::count(3)));
        assert_eq!(a.has_chrome(id), Some(false), "{kind:?} adorned");
    }
}

// ── The badge's own size ─────────────────────────────────────────────────────

/// A button with no badge has no badge box — absence is absence, not a
/// zero-sized plate that later divides by its own height.
#[test]
fn a_button_without_a_badge_has_no_badge_box() {
    let mut a = harness();
    let id = button(&mut a, "Apply");
    assert_eq!(a.badge_size(id), None);
    assert_eq!(a.button_boxes(id).unwrap().badge, None);
}

/// The dot form is a fixed square; the count form is a stadium that never
/// measures narrower than it is tall, so a single digit reads as a circle
/// rather than a squashed pill.
#[test]
fn the_dot_is_square_and_the_count_never_narrower_than_it_is_tall() {
    let mut a = harness();

    let dot = button(&mut a, "Apply");
    a.apply_prop(dot, Prop::Badge, &V::Badge(Badge::dot()));
    let (dw, dh) = a.badge_size(dot).unwrap();
    assert_eq!(dw, dh, "the dot form is a square");

    let one = button(&mut a, "Apply");
    a.apply_prop(one, Prop::Badge, &V::Badge(Badge::count(7)));
    let (ow, oh) = a.badge_size(one).unwrap();
    assert!(
        ow >= oh,
        "a one-digit count floors at a circle, got {ow}x{oh}"
    );
    assert!(oh > dh, "the count's plate is taller than a bare dot");
}

// ── Ornaments and the label do not overlap ───────────────────────────────────

/// The invariant every consumer depends on: whatever a button carries, the
/// label's box starts after everything leading it and ends before everything
/// trailing it. This is the property the old index-based reasoning got wrong,
/// and it is the one a reader can check by eye.
#[test]
fn ornaments_never_overlap_the_label() {
    let mut a = harness();

    let cases: Vec<(&str, Option<i32>, Option<Badge>)> = vec![
        ("icon only", Some(ICON), None),
        ("trailing count", None, Some(Badge::count(12))),
        ("leading dot", None, Some(Badge::dot().leading())),
        ("icon + trailing count", Some(ICON), Some(Badge::count(3))),
        ("icon + leading dot", Some(ICON), Some(Badge::dot().leading())),
    ];

    for (name, icon, badge) in cases {
        let id = button(&mut a, "Apply");
        if let Some(cp) = icon {
            a.apply_prop(id, Prop::Icon, &V::I32(cp));
        }
        if let Some(b) = badge {
            a.apply_prop(id, Prop::Badge, &V::Badge(b));
        }
        let b = a.button_boxes(id).unwrap();

        assert!(b.label.0 < b.label.2, "{name}: the label box inverted");
        if let Some(i) = b.icon {
            assert!(
                i.2 <= b.label.0,
                "{name}: icon ends at {} but the label starts at {}",
                i.2,
                b.label.0
            );
        }
        if let Some(g) = b.badge {
            let leading = badge.is_some_and(|x| x.leading);
            if leading {
                assert!(
                    g.2 <= b.label.0,
                    "{name}: leading badge ends at {} but the label starts at {}",
                    g.2,
                    b.label.0
                );
            } else {
                assert!(
                    g.0 >= b.label.2,
                    "{name}: trailing badge starts at {} but the label ends at {}",
                    g.0,
                    b.label.2
                );
            }
            // The plate is shorter than the control and centred in it — a
            // badge as tall as the button is not a badge.
            assert!(g.1 > 0.0 && g.3 < 32.0, "{name}: the plate is full height");
            assert!(
                (g.1 - (32.0 - (g.3 - g.1)) / 2.0).abs() < 0.51,
                "{name}: the plate is not vertically centred"
            );
        }
    }
}

/// Two ornaments with no label between them still get a gap.
///
/// The gap used to be charged per-ornament and only when there was a label, so
/// an icon-and-badge button with no words drew the two flush against each
/// other. The row is a sequence: every adjacent pair is separated, whatever the
/// pair happens to be.
#[test]
fn two_ornaments_with_no_label_are_still_separated() {
    let mut a = harness();
    let id = a.insert(K::Button).unwrap();
    a.set_rect(id, 48.0, 32.0);
    a.apply_prop(id, Prop::Icon, &V::I32(ICON));
    a.apply_prop(id, Prop::Badge, &V::Badge(Badge::dot()));

    let b = a.button_boxes(id).unwrap();
    let (icon, badge) = (b.icon.unwrap(), b.badge.unwrap());
    assert!(
        badge.0 - icon.2 > 0.0,
        "the dot starts at {} and the icon ends at {} — they are touching",
        badge.0,
        icon.2
    );
}

/// A leading badge heads the row, in front of the icon.
///
/// It is a status lamp for the whole control — "● Live" — so it reads at the
/// start of the row rather than wedged between the icon and the words it
/// qualifies.
#[test]
fn a_leading_badge_heads_the_row() {
    let mut a = harness();
    let id = button(&mut a, "Apply");
    a.apply_prop(id, Prop::Icon, &V::I32(ICON));
    a.apply_prop(id, Prop::Badge, &V::Badge(Badge::dot().leading()));

    let b = a.button_boxes(id).unwrap();
    let (icon, badge) = (b.icon.unwrap(), b.badge.unwrap());
    assert!(
        badge.2 <= icon.0,
        "the leading badge ends at {} but the icon starts at {}",
        badge.2,
        icon.0
    );
}

// ── An ornament-only button ──────────────────────────────────────────────────

/// With no words, the label box is the whole control rather than a sliver left
/// over after the ornaments — so a lone icon or badge centres as chrome instead
/// of being pushed against an edge by a gap it should never have been charged.
#[test]
fn an_ornament_only_button_keeps_the_whole_box() {
    let mut a = harness();
    let id = a.insert(K::Button).unwrap();
    a.set_rect(id, 32.0, 32.0);
    a.apply_prop(id, Prop::Icon, &V::I32(ICON));

    let b = a.button_boxes(id).unwrap();
    assert_eq!(b.label, (0.0, 0.0, 32.0, 32.0));
}

// ── The tint is the plate's, not the label's ─────────────────────────────────

/// A tinted badge changes only the badge. The tint reaching the button's own
/// fill would be the failure mode of storing it anywhere but on the badge.
#[test]
fn a_tint_does_not_disturb_the_geometry() {
    let mut a = harness();

    let plain = button(&mut a, "Inbox");
    a.apply_prop(plain, Prop::Badge, &V::Badge(Badge::count(9)));

    let tinted = button(&mut a, "Inbox");
    a.apply_prop(
        tinted,
        Prop::Badge,
        &V::Badge(Badge::count(9).tint(Color::rgb(255, 0, 128))),
    );

    assert_eq!(a.badge_size(plain), a.badge_size(tinted));
    assert_eq!(
        a.button_boxes(plain).unwrap(),
        a.button_boxes(tinted).unwrap()
    );
}

/// Unsetting the badge returns the geometry to exactly what a button that never
/// had one resolves — the reset contract in `prop_reset.rs`, restated where it
/// is actually visible.
#[test]
fn unsetting_the_badge_restores_the_label_box() {
    let mut a = harness();
    let id = button(&mut a, "Inbox");
    let bare = a.button_boxes(id).unwrap();

    a.apply_prop(id, Prop::Badge, &V::Badge(Badge::count(42)));
    assert_ne!(a.button_boxes(id).unwrap(), bare);

    a.apply_prop(id, Prop::Badge, &V::Unset);
    assert_eq!(a.button_boxes(id).unwrap(), bare);
}
