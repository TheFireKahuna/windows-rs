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

use windows_reactor::dcomp_test_api::{ArenaHarness, NavHit, PartPlanProbe};
use windows_reactor::{
    Color, ControlId, ControlKind as K, NavViewItem, Prop, PropValue as V, Thickness,
    selector_bar_item,
};

/// Below-band slot roles, mirroring `parts.rs`.
const TRAY_FILL: usize = 0;
const PILL: usize = 2;
const SEG_INK: usize = 3;
/// The pane's band, after the divider joined it at index 1: the hairline
/// between pane and content was the last thing the NavigationView painted, and
/// it is a part now that the control owns no surface.
const NAV_DIVIDER: usize = 1;
const NAV_TILE: usize = 2;
const NAV_BAR: usize = 3;
const SET_TILE: usize = 4;
/// Above-band: the row wash, then the chrome-button wash.
const NAV_CHROME_INK: usize = 1;

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

/// The divider is a part, and it stands at the pane's trailing edge.
///
/// It was the last thing the NavigationView painted, and the pane's surface went
/// away with it. A hairline that landed anywhere but on `metrics.width` would
/// read as the pane being a different width than the content beside it thinks it
/// is — the two are laid out from that one number.
#[test]
fn the_divider_stands_at_the_panes_trailing_edge() {
    let mut a = harness();
    let plan = pane(&mut a, 320.0, 400.0, &["One", "Two"], 0);
    let bg = plan.below[0]
        .expect("the background slot is always planned")
        .rect
        .expect("a pane always places its background");
    let div = plan.below[NAV_DIVIDER]
        .expect("the divider slot is always planned")
        .rect
        .expect("a pane always places its divider");

    assert_eq!(
        div.0, bg.2,
        "the divider must stand at the pane's width, got {div:?} against a {bg:?} background",
    );
    assert_eq!(div.1, 0.0, "the divider runs from the pane's top…");
    assert_eq!(div.3, bg.3, "…to its full height");
    assert!(
        div.2 > 0.0 && div.2 < 2.0,
        "a divider is a hairline, got {} DIPs wide",
        div.2,
    );
    assert!(
        plan.below[NAV_DIVIDER].unwrap().glides,
        "the divider must ride the same glide as the background, or the pane's \
         edge separates into two lines as it opens",
    );
}

/// The chrome wash is a second part, not the row ink moved.
///
/// The two never share a sprite: a chrome button's wash is a 40-DIP rounded
/// square at the head of the pane and a row's is a full-width tile, so one part
/// serving both would size itself wrong for whichever it was not built for.
#[test]
fn a_hovered_chrome_button_washes_on_its_own_part() {
    let mut a = harness();
    let id = a.insert(K::NavigationView).unwrap();
    a.apply_prop(
        id,
        Prop::MenuItems,
        &V::NavMenuItems(vec![NavViewItem::new("One"), NavViewItem::new("Two")]),
    );
    a.apply_prop(id, Prop::SelectedIndex, &V::I32(0));
    a.set_rect(id, 320.0, 400.0);

    // Nothing hovered: neither wash has a target.
    let rest = a.part_plan(id, 1.0).unwrap();
    assert_eq!(
        rest.above[NAV_CHROME_INK].unwrap().rect,
        None,
        "an unhovered pane must not place its chrome wash",
    );

    // The hamburger, at the pane's own negative hot sentinel.
    a.set_nav_hot(id, Some(NavHit::Toggle));
    let hot = a.part_plan(id, 1.0).unwrap();
    let chrome = hot.above[NAV_CHROME_INK]
        .expect("the chrome wash slot is always planned")
        .rect
        .expect("a hovered chrome button must place the wash");
    assert_eq!(chrome.1, 0.0, "the chrome row is at the pane's head");
    assert!(
        chrome.3 <= 40.0,
        "the chrome wash is a button-sized square, not a row tile, got {chrome:?}",
    );
    assert_eq!(
        hot.above[0].unwrap().rect,
        None,
        "hovering a chrome button must leave the ROW ink unplaced — the two are \
         separate parts and a chrome hover is not on any row",
    );
    assert!(
        !hot.above[NAV_CHROME_INK].unwrap().glides,
        "the chrome wash must snap: the two buttons sit side by side, and a \
         glide between them would wash the gap they share",
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

/// Indeterminate hands the lane to the sweep: the plan BINDS the sweep's source
/// but does not place it, and the determinate fill hides without being moved.
///
/// The two halves used to be conflated. This test asserted the sweep slot was
/// absent from the plan entirely — "it is a forever animation, not a
/// placement" — which is right about the POSITION and wrong about the BRUSH. An
/// unplanned slot is one `apply_band` never binds a source to, so the sweep
/// sprite had nothing to draw: `progress_sweep` placed it and looped it
/// perfectly and invisibly, and an indeterminate bar showed an empty lane for as
/// long as it had been indeterminate. Only a bar that had been determinate
/// first — and so had been given a source by the fill's branch — ever showed a
/// travelling segment at all.
///
/// So: the slot is planned, with `rect: None`, which binds the source while
/// leaving `Offset.X` to the loop that owns it.
#[test]
fn indeterminate_yields_the_lane_to_the_sweep() {
    let mut a = harness();
    let ind = progress(&mut a, 0.5, true);

    let fill = ind.below[1].unwrap();
    assert_eq!(fill.rect, None, "the determinate fill must not be placed");
    assert_eq!(fill.opacity, 0.0, "the determinate fill must be hidden");

    let sweep = ind.below[2].expect(
        "the sweep slot must be PLANNED so a source is bound to it — an unplanned \
         slot is a sprite with no brush, which loops invisibly",
    );
    assert!(
        sweep.key_fingerprint.is_some(),
        "the sweep must bind an atlas source, or there is nothing to see travel",
    );
    assert_eq!(
        sweep.rect, None,
        "the sweep must not be placed by the plan — the forever loop owns Offset.X",
    );

    // ...and a determinate bar claims it back, hidden and not moved.
    let det = progress(&mut a, 0.5, false);
    assert!(det.below[2].is_some(), "a determinate bar hides the sweep slot");
    assert_eq!(det.below[2].unwrap().rect, None, "hiding must not move it");
}

// ── Button family ────────────────────────────────────────────────────────────

/// Focus rings sit OUTSIDE the control's bounds — an inset ring eats into the
/// button's own face — and they exist only while the focus VISUAL is asked for,
/// which is not the same as having focus (a pointer press focuses silently).
#[test]
fn focus_rings_sit_outside_the_button_and_only_when_ringed() {
    let mut a = harness();
    let id = a.insert(K::Button).unwrap();
    a.set_rect(id, 120.0, 32.0);

    let blurred = a.part_plan(id, 1.0).unwrap();
    assert_eq!(blurred.above[1].unwrap().rect, None, "no ring while blurred");
    assert_eq!(blurred.above[1].unwrap().opacity, 0.0);

    a.set_focus_ring(id, true);
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

/// An InfoBadge laid out at `w` x `h`, carrying `count` (or the bare dot).
fn badge(a: &mut ArenaHarness, w: f32, h: f32, count: Option<i32>) -> PartPlanProbe {
    let id = a.insert(K::InfoBadge).unwrap();
    if let Some(c) = count {
        a.apply_prop(id, Prop::Value, &V::I32(c));
    }
    a.set_rect(id, w, h);
    a.part_plan(id, 1.0).unwrap()
}

/// The plate fills the pill, and the dot is a CENTRED SQUARE inside whatever box
/// it was given.
///
/// The dot's `min(w, h)` is the whole reason `info_badge::plate_box` exists: a
/// badge in a flex row can be handed a box of any shape, and a dot stretched to
/// it stops being a dot. Asserting the rect is the only way to see that — the
/// plate is a retained sprite, so nothing about it appears in a repaint.
#[test]
fn the_plate_fills_a_count_and_stays_round_for_a_dot() {
    let mut a = harness();

    let pill = badge(&mut a, 40.0, 16.0, Some(128));
    assert_eq!(
        pill.below[0].as_ref().unwrap().rect,
        Some((0.0, 0.0, 40.0, 16.0)),
        "a numeric badge's plate is its whole box",
    );

    // Deliberately oblong: the dot must not take the width.
    let dot = badge(&mut a, 40.0, 16.0, None);
    assert_eq!(
        dot.below[0].as_ref().unwrap().rect,
        Some((12.0, 0.0, 16.0, 16.0)),
        "a dot is a centred square, not a stretched pill",
    );
}

/// A badge is neither focusable nor interactive, so its plan carries nothing
/// above it — no ring, no hover ink.
///
/// Worth stating because every other converted kind DOES have an upper band, and
/// copying one of them is how a badge would quietly acquire a focus ring it can
/// never legitimately show.
#[test]
fn a_badge_has_no_ring_and_no_ink() {
    let mut a = harness();
    let p = badge(&mut a, 40.0, 16.0, Some(3));
    assert!(
        p.above.iter().all(|s| s.is_none()),
        "a badge takes no focus ring and no hover wash",
    );
}

/// The badge reaches no `BeginDraw` at all: plate below, count as glyph sprites
/// above, and nothing left for a surface to hold.
///
/// This is the claim the whole conversion is FOR, and it is one boolean — so
/// without it, the badge could regress to allocating a surface per instance and
/// every other test here would still pass.
#[test]
fn a_badge_owns_no_surface() {
    let mut a = harness();
    for count in [None, Some(3)] {
        let id = a.insert(K::InfoBadge).unwrap();
        if let Some(c) = count {
            a.apply_prop(id, Prop::Value, &V::I32(c));
        }
        a.set_rect(id, 40.0, 16.0);
        assert_eq!(
            a.has_chrome(id),
            Some(false),
            "an InfoBadge ({count:?}) must never be given a paint surface",
        );
    }
}

/// The checkbox's focus ring wraps the WHOLE control — box, gap and label —
/// not just the box.
///
/// WinUI rings the control, and the label is part of the control: it is inside
/// the hit target and clicking it toggles. A ring cut to the 18-DIP box would
/// read as ringing an icon that happens to sit next to some text.
#[test]
fn the_checkbox_ring_wraps_the_label_not_just_the_box() {
    let mut a = harness();
    let id = a.insert(K::CheckBox).unwrap();
    a.set_rect(id, 180.0, 24.0);

    // Blurred: no ring at all. Focus alone must not draw one — only a Tab does.
    let blurred = a.part_plan(id, 1.0).unwrap();
    assert!(
        blurred.above[1].as_ref().is_none_or(|s| s.opacity == 0.0),
        "an unringed checkbox must show no focus ring",
    );

    a.set_focus_ring(id, true);
    let ringed = a.part_plan(id, 1.0).unwrap();
    let (_, _, w, _) = ringed.above[1]
        .as_ref()
        .and_then(|s| s.rect)
        .expect("a ringed checkbox places its inner ring");
    assert!(
        w > 180.0,
        "the ring spans the whole control (got {w} for a 180 DIP box), \
         so it must be wider than the node, not cut to the 18 DIP box",
    );
}

/// A checkbox reaches no `BeginDraw`: fill, outline, checkmark and ring are
/// parts, and the label is glyph sprites.
///
/// The outline was the last thing drawing, and because it is hover-brightened
/// it dragged the label through a repaint on every pointer crossing.
#[test]
fn a_checkbox_owns_no_surface() {
    let mut a = harness();
    let id = a.insert(K::CheckBox).unwrap();
    a.set_rect(id, 180.0, 24.0);
    assert_eq!(a.has_chrome(id), Some(false));
}

/// The Expander's ring encloses the HEADER STRIP, not the expanded node.
///
/// Only the header is chrome — the content below it is ordinary layout — so a
/// ring cut to the node would grow as the control expands and read as a group
/// box drawn around the content rather than as focus on the thing Space
/// activates.
#[test]
fn the_expander_ring_stays_on_the_header_however_tall_the_content() {
    let mut a = harness();
    let id = a.insert(K::Expander).unwrap();
    a.set_focus_ring(id, true);

    // Collapsed: node is the header.
    a.set_rect(id, 300.0, 40.0);
    let short = a.part_plan(id, 1.0).unwrap();
    let (_, _, _, h1) = short.above[1].as_ref().and_then(|s| s.rect).expect("ring placed");

    // Expanded: the node is far taller, the header is not.
    a.set_rect(id, 300.0, 400.0);
    let tall = a.part_plan(id, 1.0).unwrap();
    let (_, _, _, h2) = tall.above[1].as_ref().and_then(|s| s.rect).expect("ring placed");

    assert_eq!(
        h1, h2,
        "the ring must not grow with the content ({h1} then {h2})",
    );
    assert!(h2 < 400.0, "a ring the height of the node would enclose the content");
}

/// An expander reaches no `BeginDraw`: header fill, border, wash and ring are
/// parts, and its label and chevron are glyph sprites.
#[test]
fn an_expander_owns_no_surface() {
    let mut a = harness();
    let id = a.insert(K::Expander).unwrap();
    a.set_rect(id, 300.0, 40.0);
    assert_eq!(a.has_chrome(id), Some(false));
}

// ─────────────────────────────────────────────────────────────────────────────
// Border — the container box, `parts::box_plan`
// ─────────────────────────────────────────────────────────────────────────────

/// A Border's two below-band slots.
const BOX_FILL: usize = 0;
const BOX_BORDER: usize = 1;

/// A Border at `w` x `h`, with whatever paint props the caller sets first.
fn boxed(a: &mut ArenaHarness, w: f32, h: f32, set: impl FnOnce(&mut ArenaHarness, ControlId)) -> PartPlanProbe {
    let id = a.insert(K::Border).unwrap();
    set(a, id);
    a.set_rect(id, w, h);
    a.part_plan(id, 1.0).unwrap()
}

/// A Border reaches no `BeginDraw`. It was the last primitive in the library
/// still rasterizing a rounded rect — and by a wide margin the most numerous,
/// since every card, panel and chip in a tree is one of these.
#[test]
fn a_border_owns_no_surface() {
    let mut a = harness();
    let id = a.insert(K::Border).unwrap();
    a.apply_prop(id, Prop::Background, &V::Color(Color::rgb(30, 30, 30)));
    a.set_rect(id, 200.0, 40.0);
    assert_eq!(
        a.has_chrome(id),
        Some(false),
        "a filled Border must bind its fill as a part, not buy a surface for it",
    );
}

/// The fill covers the whole node box.
#[test]
fn a_border_fills_its_whole_box() {
    let mut a = harness();
    let p = boxed(&mut a, 200.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::Background, &V::Color(Color::rgb(30, 30, 30)));
    });
    assert_eq!(
        p.below[BOX_FILL].as_ref().and_then(|s| s.rect),
        Some((0.0, 0.0, 200.0, 40.0)),
        "the fill is the node's own box",
    );
}

/// No background binds no source. An atlas entry that paints nothing is a
/// wasted raster and a wasted cache slot, and the 256-slot LRU is shared with
/// every other box in the tree.
#[test]
fn a_border_without_a_background_binds_nothing() {
    let mut a = harness();
    let p = boxed(&mut a, 200.0, 40.0, |_, _| {});
    assert_eq!(
        p.below[BOX_FILL].as_ref().map(|s| s.opacity),
        Some(0.0),
        "an unfilled Border must not bind a fill source",
    );
    assert_eq!(
        p.below[BOX_BORDER].as_ref().map(|s| s.opacity),
        Some(0.0),
        "…nor an outline it was never given",
    );
}

/// A brush with no thickness draws nothing — the same gate the painted
/// `draw_rounded_rect` sat behind. Without it a Border authored with a brush
/// and no width would grow an outline it never had on the painted path.
#[test]
fn a_border_outline_needs_a_thickness() {
    let mut a = harness();
    let brush_only = boxed(&mut a, 200.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::BorderBrush, &V::Color(Color::rgb(90, 90, 90)));
    });
    assert_eq!(
        brush_only.below[BOX_BORDER].as_ref().map(|s| s.opacity),
        Some(0.0),
        "a brush with no width drew nothing before and must draw nothing now",
    );

    let with_width = boxed(&mut a, 200.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::BorderBrush, &V::Color(Color::rgb(90, 90, 90)));
        a.apply_prop(id, Prop::BorderThickness, &V::Thickness(Thickness::uniform(1.0)));
    });
    assert_eq!(
        with_width.below[BOX_BORDER].as_ref().and_then(|s| s.rect),
        Some((0.0, 0.0, 200.0, 40.0)),
        "given a width, the outline takes the same box as the fill",
    );
}

/// A container has no hover, no press and no focus, so neither slot travels and
/// neither fades. Everything it can express is a prop write, and a prop write
/// marks the node dirty and re-syncs — there is no state for motion to carry.
#[test]
fn a_borders_chrome_never_moves_or_fades() {
    let mut a = harness();
    let p = boxed(&mut a, 200.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::Background, &V::Color(Color::rgb(30, 30, 30)));
        a.apply_prop(id, Prop::BorderBrush, &V::Color(Color::rgb(90, 90, 90)));
        a.apply_prop(id, Prop::BorderThickness, &V::Thickness(Thickness::uniform(1.0)));
    });
    for (i, name) in [(BOX_FILL, "fill"), (BOX_BORDER, "outline")] {
        let s = p.below[i].as_ref().expect("slot planned");
        assert!(!s.glides, "the {name} must snap — a container's box never travels");
        assert!(!s.fades, "the {name} must snap — a container has no state to fade between");
    }
}

/// A resize re-lays the box out, so `layout_sig` must carry the size. Without
/// it a Border that changed shape would spring its fill from the rect it no
/// longer has.
#[test]
fn a_border_relayouts_on_resize() {
    let mut a = harness();
    let small = boxed(&mut a, 200.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::Background, &V::Color(Color::rgb(30, 30, 30)));
    });
    let large = boxed(&mut a, 300.0, 40.0, |a, id| {
        a.apply_prop(id, Prop::Background, &V::Color(Color::rgb(30, 30, 30)));
    });
    assert_ne!(
        small.layout_sig, large.layout_sig,
        "a size change must read as a re-layout",
    );
}

// ── Text editors ─────────────────────────────────────────────────────────────
//
// The editor was the last kind painting its own box. Its run, placeholder,
// selection wash, composition rule, spin chevrons and caret were already
// sprites; the fill, the outline and the spin column's hairline were not, and
// they are what kept an FP16 surface open for every field in the tree.
//
// These assert the three decisions that conversion turns on, none of which a
// screenshot can settle reliably: a field with no spin column must HIDE that
// slot rather than leave it showing, focus must re-key the border rather than
// add a ring, and the whole family must stop asking for a surface at all.

/// An editor's below-band slots, mirroring `parts::editor_slot`.
const ED_FILL: usize = 0;
const ED_BORDER: usize = 1;
const ED_DIVIDER: usize = 2;

/// `editor::SPIN_MIN_BOX_W` — the width at which a NumberBox grows a spin column.
const SPIN_MIN_BOX_W: f32 = 72.0;

fn editor(a: &mut ArenaHarness, kind: K, w: f32, focused: bool) -> PartPlanProbe {
    let id = a.insert(kind).unwrap();
    a.set_rect(id, w, 32.0);
    a.set_focused(id, focused);
    a.part_plan(id, 1.0).unwrap()
}

/// The point of the whole change: no editor kind reaches a `BeginDraw`.
#[test]
fn no_editor_kind_asks_for_a_surface() {
    let mut a = harness();
    for kind in [K::NumberBox, K::TextBox, K::PasswordBox, K::AutoSuggestBox] {
        let id = a.insert(kind).unwrap();
        a.set_rect(id, 120.0, 32.0);
        assert_eq!(
            a.has_chrome(id),
            Some(false),
            "{kind:?} still owns a paint surface after its chrome was retained",
        );
    }
}

/// A wide NumberBox places the spin divider; a narrow one HIDES it.
///
/// Hidden, not omitted. A slot the plan does not mention keeps showing whatever
/// it last showed, so a field dragged below the threshold would keep a hairline
/// with no chevrons beside it. The GUI's parametric-EQ rows are all narrow
/// centred fields, so the hidden case is the common one, not the edge.
#[test]
fn the_spin_divider_appears_only_with_the_spin_column() {
    let mut a = harness();

    let wide = editor(&mut a, K::NumberBox, SPIN_MIN_BOX_W + 48.0, false);
    let divider = wide.below[ED_DIVIDER]
        .expect("a wide NumberBox must plan its divider slot")
        .rect
        .expect("a wide NumberBox must PLACE its divider");
    let box_rect = wide.below[ED_FILL].unwrap().rect.unwrap();
    assert!(
        divider.0 > box_rect.0 && divider.0 + divider.2 <= box_rect.0 + box_rect.2 + 0.5,
        "divider {divider:?} escapes the box {box_rect:?}",
    );
    assert!(divider.2 <= 2.0, "the divider is a hairline, got width {}", divider.2);

    let narrow = editor(&mut a, K::NumberBox, SPIN_MIN_BOX_W - 12.0, false);
    assert!(
        narrow.below[ED_DIVIDER].is_none_or(|s| s.rect.is_none()),
        "a NumberBox below the spin threshold must hide its divider, got {:?}",
        narrow.below[ED_DIVIDER],
    );
}

/// Only a NumberBox has a spin column at all — a TextBox that wide must not
/// grow a hairline down its trailing edge.
#[test]
fn only_the_number_box_gets_a_divider() {
    let mut a = harness();
    for kind in [K::TextBox, K::PasswordBox, K::AutoSuggestBox] {
        let plan = editor(&mut a, kind, SPIN_MIN_BOX_W + 48.0, false);
        assert!(
            plan.below[ED_DIVIDER].is_none_or(|s| s.rect.is_none()),
            "{kind:?} has no spin column and must not plan a divider",
        );
    }
}

/// Focus RE-KEYS the border — it does not add a ring.
///
/// An editor is the one converted kind whose focus affordance is not the
/// retained double-ring: it thickens its outline to the accent, which is WinUI's
/// own TextBox visual and what the painter drew. Both halves matter. If the
/// border stopped re-keying, a focused field would look unfocused; if a ring
/// slot appeared, it would show two focus signals at once.
#[test]
fn focus_rekeys_the_border_and_adds_no_ring() {
    let mut a = harness();
    let rest = editor(&mut a, K::TextBox, 160.0, false);
    let focused = editor(&mut a, K::TextBox, 160.0, true);

    let (r, f) = (rest.below[ED_BORDER].unwrap(), focused.below[ED_BORDER].unwrap());
    assert!(
        r.key_fingerprint.is_some() && f.key_fingerprint.is_some(),
        "the border slot must bind a source in both states",
    );
    assert_ne!(
        r.key_fingerprint, f.key_fingerprint,
        "focus must re-key the border (accent, thicker); it did not change",
    );
    assert_eq!(
        r.rect, f.rect,
        "focus is a re-bind, not a move — the border box must not shift",
    );

    // The fill is state-independent: only the outline reacts to focus.
    assert_eq!(
        rest.below[ED_FILL].unwrap().key_fingerprint,
        focused.below[ED_FILL].unwrap().key_fingerprint,
        "focus must not disturb the box fill",
    );

    for plan in [&rest, &focused] {
        assert!(
            plan.above.iter().all(|s| s.is_none()),
            "an editor plans NO above-band slots — the caret is inserted at the \
             top of the container after `parts::sync`, so an above slot minted \
             later would hide it",
        );
    }
}
