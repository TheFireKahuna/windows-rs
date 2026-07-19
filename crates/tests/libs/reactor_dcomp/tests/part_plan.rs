//! `backend/dcomp/parts.rs` — where a control decides its retained chrome goes.
//!
//! A control's sprites (the SelectorBar's accent pill, the NavigationView's
//! selection tile and rail bar) are *retained* compositor visuals, not painted
//! pixels. Nothing in a repaint redraws them; a sync decides a rect and asks the
//! compositor to move a sprite there. So "the pill is on the selected segment"
//! is a claim about a decision, and until that decision was separated from the
//! `SetOffset` that carries it out, no test could make the claim at all — the
//! decision needed a GPU device and an HWND to reach.
//!
//! That gap had teeth. The pill and the labels read the SAME `selected_index`,
//! so no state-level assertion could ever catch them disagreeing — and they
//! disagreed anyway, because the sprite half carried a hand-maintained shadow of
//! the selection that could be committed on a path which never placed the pill.
//! The whole class was invisible to 100+ green tests and visible to anyone
//! clicking the control.
//!
//! These tests assert the decision:
//!
//! * the indicator lands on the SELECTED segment / row, at every index; and
//! * the motion policy is right — indicators glide, hover inks snap — because a
//!   wash sliding between rows the pointer never crossed is as wrong as a pill
//!   in the wrong place.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::{ArenaHarness, PartPlanProbe};
use windows_reactor::{ControlKind as K, NavViewItem, Prop, PropValue as V, selector_bar_item};

/// Below-band slot roles, mirroring `parts.rs`.
const TRAY_FILL: usize = 0;
const PILL: usize = 2;
const SEG_INK: usize = 3;
const NAV_TILE: usize = 1;
const NAV_BAR: usize = 2;
const SET_TILE: usize = 3;

const TRACK_ON: usize = 0;
const KNOB: usize = 2;

/// `nav::SETTINGS_INDEX` — the sentinel slot the settings row selects at.
const SETTINGS_INDEX: i32 = 1 << 16;

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// A SelectorBar laid out at `w` x `h` over `labels`, with `sel` selected.
fn bar(a: &mut ArenaHarness, w: f32, h: f32, labels: &[&str], sel: i32) -> PartPlanProbe {
    let id = a.insert(K::SelectorBar).unwrap();
    a.apply_prop(
        id,
        Prop::Items,
        &V::SelectorBarItems(labels.iter().map(|s| selector_bar_item(*s)).collect()),
    );
    a.apply_prop(id, Prop::SelectedIndex, &V::I32(sel));
    a.set_rect(id, w, h);
    // The segment edges are derived from MEASURED label runs; without this the
    // widths fall back to an even split and the test asserts a degenerate case.
    a.rebuild_text(id);
    a.part_plan(id, 1.0).unwrap()
}

/// A ToggleSwitch in the given state, laid out at its usual row size.
fn toggle(a: &mut ArenaHarness, on: bool) -> PartPlanProbe {
    let id = a.insert(K::ToggleSwitch).unwrap();
    a.apply_prop(id, Prop::IsOn, &V::Bool(on));
    a.set_rect(id, 40.0, 32.0);
    a.part_plan(id, 1.0).unwrap()
}

/// A ProgressBar at `frac` (0..1), determinate or not.
fn progress(a: &mut ArenaHarness, frac: f64, indeterminate: bool) -> PartPlanProbe {
    let id = a.insert(K::ProgressBar).unwrap();
    a.apply_prop(id, Prop::Value, &V::F64(frac * 100.0));
    if indeterminate {
        a.apply_prop(id, Prop::IsIndeterminate, &V::Bool(true));
    }
    a.set_rect(id, 200.0, 8.0);
    a.part_plan(id, 1.0).unwrap()
}

/// A NavigationView laid out at `w` x `h` with `items` rows, `sel` selected.
fn pane(a: &mut ArenaHarness, w: f32, h: f32, items: &[&str], sel: i32) -> PartPlanProbe {
    let id = a.insert(K::NavigationView).unwrap();
    a.apply_prop(
        id,
        Prop::MenuItems,
        &V::NavMenuItems(items.iter().map(|s| NavViewItem::new(*s)).collect()),
    );
    a.apply_prop(id, Prop::SelectedIndex, &V::I32(sel));
    a.set_rect(id, w, h);
    a.part_plan(id, 1.0).unwrap()
}

// ── The indicator follows the selection ──────────────────────────────────────

/// THE regression this file exists for: the pill sits over the selected
/// segment, for every selection, not just the one the control was born with.
///
/// Asserted as containment against the tray rather than as literal coordinates —
/// the exact edges depend on the measured label widths, which are a font's
/// business, but "segment 1's pill starts right of segment 0's" is the property
/// that was actually broken.
#[test]
fn the_pill_lands_on_the_selected_segment() {
    let mut a = harness();
    let mut lefts = Vec::new();
    for sel in 0..3 {
        let plan = bar(&mut a, 300.0, 32.0, &["Simple", "Pro", "Expert"], sel);
        let pill = plan.below[PILL]
            .expect("the pill slot is always planned")
            .rect
            .expect("a selected segment must place the pill");
        let tray = plan.below[TRAY_FILL].unwrap().rect.unwrap();
        assert!(
            pill.0 >= tray.0 && pill.0 + pill.2 <= tray.0 + tray.2 + 0.5,
            "sel {sel}: pill {pill:?} escapes the tray {tray:?}",
        );
        lefts.push(pill.0);
    }
    assert!(
        lefts[0] < lefts[1] && lefts[1] < lefts[2],
        "the pill must advance with the selection, got lefts {lefts:?}",
    );
}

/// The same property for the pane: tile and rail bar both track the selected
/// row, and both move together.
#[test]
fn the_nav_tile_and_bar_land_on_the_selected_row() {
    let mut a = harness();
    let mut tops = Vec::new();
    for sel in 0..3 {
        let plan = pane(&mut a, 320.0, 400.0, &["One", "Two", "Three"], sel);
        let tile = plan.below[NAV_TILE].unwrap().rect.expect("selected row places the tile");
        let bar = plan.below[NAV_BAR].unwrap().rect.expect("selected row places the bar");
        assert!(
            bar.1 >= tile.1 && bar.1 + bar.3 <= tile.1 + tile.3,
            "sel {sel}: the rail bar {bar:?} must sit within its tile {tile:?}",
        );
        tops.push(tile.1);
    }
    assert!(
        tops[0] < tops[1] && tops[1] < tops[2],
        "the tile must descend with the selection, got tops {tops:?}",
    );
}

/// Nothing selected hides the indicator rather than parking it on segment 0 —
/// the `max(0)` clamp in the pill's index makes "-1" and "0" look alike, so this
/// pins the one place they must not.
#[test]
fn no_selection_hides_the_nav_indicator() {
    let mut a = harness();
    let plan = pane(&mut a, 320.0, 400.0, &["One", "Two"], -1);
    let tile = plan.below[NAV_TILE].unwrap();
    assert_eq!(tile.rect, None, "an unselected pane must not place its tile");
    assert_eq!(tile.opacity, 0.0, "an unselected pane must not show its tile");
}

// ── Motion policy ────────────────────────────────────────────────────────────

/// Indicators glide and hover inks snap. An ink that glided would wash across
/// segments the pointer never crossed; a pill that snapped would lose the
/// affordance the control exists to give.
#[test]
fn indicators_glide_and_inks_snap() {
    let mut a = harness();
    let seg = bar(&mut a, 300.0, 32.0, &["Simple", "Pro"], 0);
    assert!(seg.below[PILL].unwrap().glides, "the pill must glide between segments");
    assert!(!seg.below[SEG_INK].unwrap().glides, "the hover ink must snap");
    assert!(seg.below[SEG_INK].unwrap().fades, "the hover ink must fade");
    assert!(!seg.below[TRAY_FILL].unwrap().glides, "the tray must not travel");

    let nav = pane(&mut a, 320.0, 400.0, &["One", "Two"], 0);
    assert!(nav.below[NAV_TILE].unwrap().glides, "the selection tile must glide");
    assert!(nav.above[0].unwrap().fades, "the row hover ink must fade");
    assert!(!nav.above[0].unwrap().glides, "the row hover ink must snap");
}

// ── The layout signature ─────────────────────────────────────────────────────

/// A re-measured label changes the segment boundaries, so the signature must
/// move with it: the pill has to JUMP to boundaries that shifted under it rather
/// than slide to them, and the signature is the only thing that says so.
#[test]
fn relabelling_moves_the_segmented_layout_signature() {
    let mut a = harness();
    let short = bar(&mut a, 300.0, 32.0, &["A", "B"], 0);
    let long = bar(&mut a, 300.0, 32.0, &["A much longer label", "B"], 0);
    assert_ne!(
        short.layout_sig, long.layout_sig,
        "segment edges changed, so the layout signature must too",
    );
}

/// The settings row is a pane-height below the menu list, so ONE indicator
/// travelling between them falls past every row on the way. Each region owns its
/// indicator instead: the inactive one is hidden and — critically — is not moved,
/// so returning to it later resumes from the row it was last on.
#[test]
fn the_settings_row_has_its_own_indicator() {
    let mut a = harness();

    let menu = pane(&mut a, 320.0, 400.0, &["One", "Two"], 1);
    assert!(menu.below[NAV_TILE].unwrap().rect.is_some(), "menu tile shows for a menu row");
    assert_eq!(
        menu.below[SET_TILE].unwrap().rect,
        None,
        "the settings tile must not be placed while a menu row is selected",
    );
    assert_eq!(menu.below[SET_TILE].unwrap().opacity, 0.0);

    let settings = pane(&mut a, 320.0, 400.0, &["One", "Two"], SETTINGS_INDEX);
    let set_tile = settings.below[SET_TILE].unwrap();
    assert!(set_tile.rect.is_some(), "the settings tile shows for the settings row");
    assert_eq!(
        settings.below[NAV_TILE].unwrap().rect,
        None,
        "the menu tile must not travel down to the settings row",
    );

    // The settings indicator really is at the foot, well clear of the menu rows —
    // which is the distance that made one shared indicator wrong.
    let menu_top = menu.below[NAV_TILE].unwrap().rect.unwrap().1;
    assert!(
        set_tile.rect.unwrap().1 > menu_top + 100.0,
        "the settings row should sit far below the menu list",
    );

    // Both fade, so the handoff between regions is a cross-fade in place.
    assert!(
        menu.below[NAV_TILE].unwrap().fades && settings.below[SET_TILE].unwrap().fades,
        "region handoff must cross-fade, not pop",
    );
}

// ── ToggleSwitch ─────────────────────────────────────────────────────────────

/// The knob must actually TRAVEL between the two ends, and it must glide doing
/// it. The `parts.on` shadow made a flip start a glide and then take the
/// authoritative `place` branch on the next sync, stopping the spring dead — so
/// the knob appeared to jump rather than slide.
#[test]
fn the_toggle_knob_travels_between_the_ends_and_glides() {
    let mut a = harness();
    let off = toggle(&mut a, false);
    let on = toggle(&mut a, true);

    let kx = |p: &PartPlanProbe| p.below[KNOB].unwrap().rect.unwrap().0;
    assert!(
        kx(&on) > kx(&off),
        "the knob must move right when switched on: {} then {}",
        kx(&off),
        kx(&on),
    );
    assert!(on.below[KNOB].unwrap().glides, "the knob must glide, not jump");

    // The tracks carry the state as opacity and never travel.
    assert!(!on.below[TRACK_ON].unwrap().glides, "the track must not travel");
    assert!(on.below[TRACK_ON].unwrap().fades, "the tracks cross-fade");
    assert_eq!(
        off.below[TRACK_ON].unwrap().rect,
        on.below[TRACK_ON].unwrap().rect,
        "both track sprites sit at the same place in both states",
    );
    assert!(
        on.below[TRACK_ON].unwrap().opacity > off.below[TRACK_ON].unwrap().opacity,
        "the accent track shows when on",
    );
}

// ── ProgressBar ──────────────────────────────────────────────────────────────

/// The gap this file could not reach until the determinate fill became
/// plan-driven. `progress_sync` compared `parts.frac` and took the
/// authoritative `place` branch when it matched, so the fill STEPPED to each new
/// length instead of growing into it.
#[test]
fn the_progress_fill_grows_with_the_value_and_glides() {
    let mut a = harness();
    let quarter = progress(&mut a, 0.25, false);
    let half = progress(&mut a, 0.5, false);

    let fw = |p: &PartPlanProbe| p.below[1].unwrap().rect.unwrap().2;
    assert!(
        fw(&half) > fw(&quarter),
        "the fill must lengthen with the value: {} then {}",
        fw(&quarter),
        fw(&half),
    );
    assert!(half.below[1].unwrap().glides, "the fill must grow, not step");
    assert!(!half.below[0].unwrap().glides, "the track never travels");
}

/// Indeterminate hands the lane to the sweep. The sweep slot is deliberately
/// ABSENT from the plan while it runs — it is a forever animation, not a
/// placement — and the determinate fill hides without being moved.
#[test]
fn indeterminate_yields_the_lane_to_the_sweep() {
    let mut a = harness();
    let ind = progress(&mut a, 0.5, true);

    let fill = ind.below[1].unwrap();
    assert_eq!(fill.rect, None, "the determinate fill must not be placed");
    assert_eq!(fill.opacity, 0.0, "the determinate fill must be hidden");
    assert!(
        ind.below[2].is_none(),
        "the sweep slot must be left alone while the loop owns it",
    );

    // ...and a determinate bar claims it back, hidden and not moved.
    let det = progress(&mut a, 0.5, false);
    assert!(det.below[2].is_some(), "a determinate bar hides the sweep slot");
    assert_eq!(det.below[2].unwrap().rect, None, "hiding must not move it");
}

// ── Button family ────────────────────────────────────────────────────────────

/// Focus rings sit OUTSIDE the control's bounds — an inset ring eats into the
/// button's own face — and they exist only while focused.
#[test]
fn focus_rings_sit_outside_the_button_and_only_when_focused() {
    let mut a = harness();
    let id = a.insert(K::Button).unwrap();
    a.set_rect(id, 120.0, 32.0);

    let blurred = a.part_plan(id, 1.0).unwrap();
    assert_eq!(blurred.above[1].unwrap().rect, None, "no ring while blurred");
    assert_eq!(blurred.above[1].unwrap().opacity, 0.0);

    a.set_focused(id, true);
    let focused = a.part_plan(id, 1.0).unwrap();
    let inner = focused.above[1].unwrap().rect.expect("focused places the inner ring");
    let outer = focused.above[2].unwrap().rect.expect("focused places the outer ring");

    assert!(inner.0 < 0.0 && inner.1 < 0.0, "the ring must sit outside the box: {inner:?}");
    assert!(
        outer.0 < inner.0 && outer.2 > inner.2,
        "the outer ring must enclose the inner one: {outer:?} vs {inner:?}",
    );
}

// ── Distance-aware duration ──────────────────────────────────────────────────

/// A spring settles in the same time whatever the distance, so without this a
/// long move does not take longer — it travels faster, which is what made the
/// pane-height trip look thrown. Duration must rise with distance, SUB-linearly,
/// and be clamped at both ends.
#[test]
fn glide_duration_rises_sublinearly_with_distance() {
    let short = windows_reactor::dcomp_test_api::chrome_spring_period(60.0);
    let long = windows_reactor::dcomp_test_api::chrome_spring_period(600.0);

    assert!(long > short, "a longer move must take longer, got {short} then {long}");
    let ratio = long / short;
    assert!(
        (2.0..5.0).contains(&ratio),
        "10x the distance should take ~3x the time, not {ratio}x — linear scaling drags, \
         and no scaling is what threw the tile",
    );
}

/// Clamped at both ends: a hairline move is still motion, and an enormous one
/// still finishes.
#[test]
fn glide_duration_is_clamped_at_both_ends() {
    let tiny = windows_reactor::dcomp_test_api::chrome_spring_period(0.5);
    let huge = windows_reactor::dcomp_test_api::chrome_spring_period(100_000.0);
    let reference = windows_reactor::dcomp_test_api::chrome_spring_period(60.0);

    assert!(tiny > 0.0, "no glide may be instant");
    assert!(tiny >= reference * 0.5, "a tiny move must not collapse to nothing");
    assert!(huge <= reference * 4.0, "a huge move must not drag: {huge} vs {reference}");
}

/// The pane's WIDTH is the open/close animation and must not force a snap; its
/// HEIGHT is a resize and must. One signature, and only the height is in it.
#[test]
fn the_pane_signature_tracks_height_not_width() {
    let mut a = harness();
    let narrow = pane(&mut a, 48.0, 400.0, &["One", "Two"], 0);
    let wide = pane(&mut a, 320.0, 400.0, &["One", "Two"], 0);
    assert_eq!(
        narrow.layout_sig, wide.layout_sig,
        "a width change is the pane opening — it must glide, not snap",
    );

    let taller = pane(&mut a, 320.0, 600.0, &["One", "Two"], 0);
    assert_ne!(
        wide.layout_sig, taller.layout_sig,
        "a height change is a resize — it must snap",
    );
}
