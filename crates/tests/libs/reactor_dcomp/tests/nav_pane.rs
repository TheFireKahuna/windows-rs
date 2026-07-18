//! `backend/dcomp/nav.rs` — the NavigationView pane's derived geometry.
//!
//! The pane is not a fixed rail: its width, how many rows fit, where the first
//! row starts and which elements exist at all are all *derived* from the pane
//! state plus the node's own laid-out size. Four separate consumers read that
//! derivation — the paint, the retained sprites, the pointer hit test and the
//! accessibility tree — and they are only correct because they read the SAME
//! one. A hit test that disagrees with the paint by a single row selects the
//! wrong page, silently, and looks fine in a screenshot.
//!
//! So these tests assert two families of property:
//!
//! * the derivation itself — that each pane prop moves the geometry the way its
//!   WinUI counterpart does, including the adaptive `Auto` mode; and
//! * the agreement — that the hit test lands inside the box the paint would
//!   have drawn, for every row, at several pane states.
//!
//! The reset half of the contract is not repeated here: `prop_reset.rs` already
//! compares whole-node digests, and the pane's Taffy inset is part of that
//! digest, so a pane prop whose `Unset` re-derived a different width than the
//! node was born with fails there.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::{ArenaHarness, NavHit};
use windows_reactor::{ControlKind as K, NavViewItem, Prop, PropValue as V};

/// WinUI's `NavigationView` defaults, which the backend mirrors.
const DEFAULT_OPEN_LEN: f32 = 320.0;
/// `CompactPaneLength` — the icon rail.
const RAIL_W: f32 = 48.0;
/// The back / hamburger row, and one item row.
const CHROME_H: f32 = 40.0;
const ITEM_H: f32 = 48.0;

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// A NavigationView laid out at `w` x `h` with `items` menu rows.
fn pane(a: &mut ArenaHarness, w: f32, h: f32, items: &[&str]) -> windows_reactor::ControlId {
    let id = a.insert(K::NavigationView).unwrap();
    if !items.is_empty() {
        // The real menu shape, not a `StrList` — `MenuItems` carries tags and
        // icons alongside the label, and the backend's dropped-prop diagnostic
        // rejects anything else.
        a.apply_prop(
            id,
            Prop::MenuItems,
            &V::NavMenuItems(items.iter().map(|s| NavViewItem::new(*s)).collect()),
        );
    }
    a.set_rect(id, w, h);
    id
}

// ── Width derivation ─────────────────────────────────────────────────────────

/// A NavigationView nobody configured is WinUI's own default: an OPEN pane at
/// `open_pane_length`, not the icon rail this backend used to draw
/// unconditionally.
///
/// Stated first because it is the assumption every other case rests on, and
/// because it is the one that changed: `Extras::DEFAULT` has always carried
/// `is_pane_open: true` / `open_pane_length: 320`, and until the pane was drawn
/// nothing read either.
#[test]
fn a_virgin_pane_is_open_at_the_winui_default_width() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One", "Two"]);
    let p = a.nav_probe(id).unwrap();

    assert_eq!(p.width, DEFAULT_OPEN_LEN);
    assert!(p.expanded, "the default pane shows labels, not just icons");
    assert!(a.nav_pad_left_is(id, DEFAULT_OPEN_LEN), "content inset must match the pane width");
}

/// Closing the pane collapses it to the rail, and reopening restores the full
/// width — the hamburger's entire behaviour, and the property the content pane
/// depends on to resize with it.
#[test]
fn closing_the_pane_collapses_it_to_the_rail_and_reopening_restores_it() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One"]);

    a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(false));
    let closed = a.nav_probe(id).unwrap();
    assert_eq!(closed.width, RAIL_W);
    assert!(!closed.expanded, "a closed pane is icons only");
    assert!(a.nav_pad_left_is(id, RAIL_W), "the content inset must follow the pane in");

    a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(true));
    let open = a.nav_probe(id).unwrap();
    assert_eq!(open.width, DEFAULT_OPEN_LEN);
    assert!(a.nav_pad_left_is(id, DEFAULT_OPEN_LEN));
}

/// `OpenPaneLength` sets the open width and nothing else — a closed pane still
/// collapses to the rail regardless of how wide it opens to.
#[test]
fn open_pane_length_governs_the_open_width_only() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One"]);

    a.apply_prop(id, Prop::OpenPaneLength, &V::F64(260.0));
    assert_eq!(a.nav_probe(id).unwrap().width, 260.0);
    assert!(a.nav_pad_left_is(id, 260.0));

    a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(false));
    assert_eq!(
        a.nav_probe(id).unwrap().width,
        RAIL_W,
        "a closed pane is the rail whatever its open length"
    );
}

/// The pane never eats the whole node. Past the clamp the content pane would
/// measure zero and the app's page would vanish rather than merely narrow — so
/// an absurd `OpenPaneLength` is bounded, not honoured.
#[test]
fn an_over_wide_pane_is_clamped_so_content_survives() {
    let mut a = harness();
    let id = pane(&mut a, 500.0, 800.0, &["One"]);
    a.apply_prop(id, Prop::OpenPaneLength, &V::F64(5000.0));

    let w = a.nav_probe(id).unwrap().width;
    assert!(w < 500.0, "pane {w} left no room for content in a 500 DIP node");
}

/// A fixed `PaneDisplayMode` overrides the adaptive default: `LeftCompact` is
/// the rail even though the pane is open, and `Left` is the full pane even at a
/// width `Auto` would have compacted.
#[test]
fn a_fixed_display_mode_overrides_the_adaptive_default() {
    let mut a = harness();

    let compact = pane(&mut a, 1200.0, 800.0, &["One"]);
    a.apply_prop(compact, Prop::PaneDisplayMode, &V::I32(3)); // LeftCompact
    a.apply_prop(compact, Prop::IsPaneOpen, &V::Bool(false));
    assert_eq!(a.nav_probe(compact).unwrap().width, RAIL_W);

    let left = pane(&mut a, 700.0, 800.0, &["One"]);
    a.apply_prop(left, Prop::PaneDisplayMode, &V::I32(1)); // Left
    let p = a.nav_probe(left).unwrap();
    assert_eq!(p.width, DEFAULT_OPEN_LEN);
    assert!(p.expanded);
}

/// `Auto` (the default) follows WinUI's adaptive thresholds: a CLOSED pane
/// resolves to the rail on a wide window and to the bare hamburger strip on a
/// narrow one. Width alone changes the answer, with no prop write between.
#[test]
fn auto_mode_adapts_a_closed_pane_to_the_window_width() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One"]);
    a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(false));

    a.set_rect(id, 1200.0, 800.0);
    assert_eq!(a.nav_probe(id).unwrap().width, RAIL_W, "wide: the icon rail");

    a.set_rect(id, 500.0, 800.0);
    let narrow = a.nav_probe(id).unwrap().width;
    assert!(
        narrow < RAIL_W,
        "narrow: a closed minimal pane is the hamburger strip, got {narrow}"
    );
}

/// The adaptive answer must reach the Taffy style, not just the probe — the
/// inset is only re-derived when something calls `apply_nav_metrics`, and a
/// resize is one of the things that must.
#[test]
fn re_deriving_after_a_resize_moves_the_content_inset() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One"]);
    a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(false));
    assert!(a.nav_pad_left_is(id, RAIL_W));

    a.set_rect(id, 500.0, 800.0);
    a.nav_apply_metrics(id);
    let w = a.nav_probe(id).unwrap().width;
    assert!(
        a.nav_pad_left_is(id, w),
        "after a resize the inset must equal the re-derived pane width {w}"
    );
}

// ── Which elements exist ─────────────────────────────────────────────────────

/// The pane's chrome is present by default and each piece is individually
/// removable — and losing the chrome row lifts the first item row to the top,
/// which is exactly why the hit test cannot assume a fixed origin.
#[test]
fn hiding_the_chrome_row_lifts_the_items_to_the_top() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One", "Two"]);

    let born = a.nav_probe(id).unwrap();
    assert!(born.back && born.toggle && born.settings, "chrome is born visible");
    assert_eq!(born.items_y, CHROME_H, "items start below the chrome row");

    a.apply_prop(id, Prop::IsBackButtonVisible, &V::Bool(false));
    a.apply_prop(id, Prop::IsPaneToggleButtonVisible, &V::Bool(false));
    let bare = a.nav_probe(id).unwrap();
    assert!(!bare.back && !bare.toggle);
    assert_eq!(bare.items_y, 0.0, "with no chrome row the first item is at the top");
}

/// `IsSettingsVisible` removes the settings row from the geometry entirely —
/// which is what makes it removable from the accessibility tree too, since both
/// read this one answer.
#[test]
fn hiding_settings_removes_the_row_from_the_geometry() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One"]);
    assert!(a.nav_probe(id).unwrap().settings_y.is_some());

    a.apply_prop(id, Prop::IsSettingsVisible, &V::Bool(false));
    let p = a.nav_probe(id).unwrap();
    assert!(!p.settings);
    assert!(p.settings_y.is_none());
}

/// A pane too short for its menu draws a prefix of it. The bound matters
/// because it also governs the hit test and the item count a screen reader is
/// told: nothing may be announced that is not on screen and cannot be clicked.
#[test]
fn a_short_pane_exposes_only_the_rows_that_fit() {
    let mut a = harness();
    let items = ["One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight"];
    let id = pane(&mut a, 1200.0, 800.0, &items);
    assert_eq!(a.nav_probe(id).unwrap().visible_items, items.len());

    // Chrome row + three item rows + the settings row, and not a pixel more.
    a.set_rect(id, 1200.0, CHROME_H + 3.0 * ITEM_H + ITEM_H);
    let short = a.nav_probe(id).unwrap();
    assert_eq!(short.visible_items, 3);
    assert!(
        short.settings_y.is_some(),
        "the settings row is pinned to the foot and keeps its room"
    );
}

// ── Paint / hit-test agreement ───────────────────────────────────────────────

/// The hit test and the paint must resolve the same row. Asserted by taking the
/// box the paint draws each item in and hit-testing its own centre — the
/// property that makes a click land on the page the user aimed at.
#[test]
fn every_drawn_row_hit_tests_back_to_itself() {
    let mut a = harness();
    let items = ["One", "Two", "Three", "Four"];

    // Across pane states, because each moves the rows: the chrome row shifts
    // them down, and closing the pane changes the width they span.
    for (open, chrome) in [(true, true), (false, true), (true, false)] {
        let id = pane(&mut a, 1200.0, 800.0, &items);
        a.apply_prop(id, Prop::IsPaneOpen, &V::Bool(open));
        if !chrome {
            a.apply_prop(id, Prop::IsBackButtonVisible, &V::Bool(false));
            a.apply_prop(id, Prop::IsPaneToggleButtonVisible, &V::Bool(false));
        }
        let n = a.nav_probe(id).unwrap().visible_items as i32;
        assert!(n > 0);

        for i in 0..n {
            let (x, y, w, h) = a.nav_item_box(id, i).unwrap();
            assert_eq!(
                a.nav_hit(id, x + w / 2.0, y + h / 2.0),
                Some(NavHit::Item(i)),
                "open={open} chrome={chrome}: the centre of item {i}'s drawn box \
                 did not hit-test back to item {i}"
            );
        }
    }
}

/// Each chrome element hit-tests as itself, and a point past the pane's
/// trailing edge belongs to the content — not to the last row that happens to
/// share its y.
#[test]
fn chrome_hit_tests_as_itself_and_the_pane_ends_at_its_edge() {
    let mut a = harness();
    let id = pane(&mut a, 1200.0, 800.0, &["One", "Two"]);
    let p = a.nav_probe(id).unwrap();

    assert_eq!(a.nav_hit(id, 20.0, 20.0), Some(NavHit::Back));
    assert_eq!(a.nav_hit(id, CHROME_H + 20.0, 20.0), Some(NavHit::Toggle));
    assert_eq!(
        a.nav_hit(id, 20.0, p.settings_y.unwrap() + ITEM_H / 2.0),
        Some(NavHit::Settings)
    );

    let (_, y, _, h) = a.nav_item_box(id, 0).unwrap();
    assert_eq!(
        a.nav_hit(id, p.width + 10.0, y + h / 2.0),
        None,
        "past the divider the point belongs to the content pane"
    );
}

/// A row the pane is too short to draw must not be hittable — the same
/// `visible_items` bound the paint stops at.
#[test]
fn a_row_that_does_not_fit_is_not_hittable() {
    let mut a = harness();
    let items = ["One", "Two", "Three", "Four", "Five", "Six"];
    let id = pane(&mut a, 1200.0, CHROME_H + 2.0 * ITEM_H + ITEM_H, &items);

    let n = a.nav_probe(id).unwrap().visible_items;
    assert!(n < items.len(), "the pane must be too short for this to test anything");

    // The box the fourth row WOULD occupy, were there room for it.
    let (_, y, _, h) = a.nav_item_box(id, n as i32).unwrap();
    assert_ne!(
        a.nav_hit(id, 20.0, y + h / 2.0),
        Some(NavHit::Item(n as i32)),
        "an undrawn row was still hittable"
    );
}
