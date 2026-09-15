//! Drives the overlay layer headless, against both halves of the focus seam.
//!
//! `Model` owns no COM, so a whole open-and-dismiss runs with no window, no device and no
//! compositor: the slot roots, the blocker entry, the focus ops and the placement all read
//! back off the patch. Reports and intents are built here rather than delivered by the
//! router, so nothing in this file needs a real press against a real window.
//!
//! The overlay layer emits focus edits and never touches a ring. [`Ring`] is the other half:
//! it holds a real [`FocusRing`] and applies what the layer emitted, so a test asserts either
//! on the ops themselves or on where focus landed once they were applied.

use super::*;
use crate::build::{Any, El, mount};
use crate::input::{FocusRing, KeyEvent, Mods, Move};
use crate::layout::Preset;
use crate::signal::live_nodes;
use crate::widget::{UiaRole, button, flyout, text};
use windows_scene::{HitEntry, HitFlags, HitTable, SinkPatch};

/// The input half of the focus seam: a ring, the ops the app half has emitted for it, and
/// the reports applying them produced.
#[derive(Default)]
struct Ring {
    ring: FocusRing,
    ops: Vec<FocusOp>,
    reports: Vec<Report>,
    hits: HitTable,
}

impl Ring {
    /// Applies every op emitted since the last call, against the array `patch` published.
    ///
    /// Leaves `reports` holding what this batch produced and `ops` empty, so a test reads
    /// each against one batch rather than against the whole run.
    fn sync(&mut self, patch: &SinkPatch) {
        self.hits.replace(patch.hit_entries());
        self.reports.clear();
        self.ring.apply(&self.ops, &self.hits, &mut self.reports);
        self.ops.clear();
    }

    /// Focuses `id` as a press would, which is how a test states where focus was before an
    /// overlay opened.
    fn seed(&mut self, id: ControlId) {
        _ = self.ring.focus(Some(id));
    }

    fn current(&self) -> Option<ControlId> {
        self.ring.current()
    }

    fn depth(&self) -> usize {
        self.ring.depth()
    }
}

/// Closes every overlay, innermost first, as a screen unmounting does.
fn close_all(overlays: &mut Overlays, focus: &mut Ring) {
    while !overlays.is_empty() {
        overlays.close_top(&mut focus.ops);
    }
}

/// Returns the scopes `ops` opens, in order.
fn pushed(ops: &[FocusOp]) -> Vec<(ScopeId, bool, Option<ControlId>)> {
    ops.iter()
        .filter_map(|op| match *op {
            FocusOp::PushScope {
                scope,
                trap,
                restore_to,
                ..
            } => Some((scope, trap, restore_to)),
            _ => None,
        })
        .collect()
}

/// Returns the scopes `ops` closes, in order.
fn popped(ops: &[FocusOp]) -> Vec<ScopeId> {
    ops.iter()
        .filter_map(|op| match *op {
            FocusOp::PopScope(scope) => Some(scope),
            _ => None,
        })
        .collect()
}

/// Returns the build module's fixture: the palette, a shaper, a fresh host, first flush
/// drained.
fn fixture() -> SinkPatch {
    crate::build::tests::fixture()
}

fn flush(patch: &mut SinkPatch) {
    Host::flush(patch);
}

fn root() -> GroupId {
    Host::with(|host| host.model().root())
}

/// Returns the hit table as the front thread would hold it, rebuilt from `patch`.
fn hits(patch: &SinkPatch) -> HitTable {
    let mut table = HitTable::default();
    table.replace(patch.hit_entries());
    table
}

fn entries(patch: &SinkPatch) -> Vec<HitEntry> {
    patch.hit_entries().to_vec()
}

/// Returns a flyout surface with two focusable rows.
fn body() -> View {
    flyout().stack((button("Alpha"), button("Beta")))
}

/// Mounts a control to anchor against and returns its mount with the id it minted.
fn invoker(patch: &mut SinkPatch) -> (Mount, ControlId) {
    let mount = mount(
        El::<Any>::seed(Preset::Bare)
            .control()
            .name("Open")
            .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, UiaRole::Button)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH)
            .row(text("Open")),
        root(),
    );
    flush(patch);
    let id = entries(patch)
        .first()
        .map(|entry| entry.id)
        .expect("the invoker declared a hit entry");
    (mount, id)
}

#[test]
fn an_overlay_contributes_a_blocker_then_itself_at_the_end_of_the_array() {
    // The array is the z-order and the scan takes the first hit from the back, so a press
    // inside resolves to the overlay and a press anywhere else to the blocker.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let content = entries(&patch).len();
    let _open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);

    let entries = entries(&patch);
    assert!(entries.len() > content + 1, "{entries:?}");
    let blocker = entries[content];
    assert!(blocker.flags.contains(HitFlags::BLOCKER));
    assert_eq!(
        (blocker.x0, blocker.y0, blocker.x1, blocker.y1),
        (0.0, 0.0, 800.0, 600.0),
        "a blocker covers the window whatever is under it"
    );
    // And it is ahead of the overlay's own entries, not behind them.
    assert!(
        entries[content + 1..]
            .iter()
            .all(|entry| !entry.flags.contains(HitFlags::BLOCKER))
    );
}

#[test]
fn an_overlay_is_placed_under_its_anchor_and_has_real_geometry() {
    // A detached root is solved with a size and an offset, so its rows carry real area in
    // the hit array rather than a zero-area entry that is visible and unhittable.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);

    let entries = entries(&patch);
    // Everything that is neither the blocker nor inside the anchor's own subtree, which is
    // not the same as "not the anchor": a control's label is an element of its own.
    let inside_anchor = |mut at: u32| {
        let mut guard = entries.len();
        while at != windows_scene::NO_ENTRY && guard > 0 {
            let Some(entry) = entries.get(at as usize) else {
                break;
            };
            if entry.id == anchor {
                return true;
            }
            at = entry.parent;
            guard -= 1;
        }
        false
    };
    let overlay: Vec<&HitEntry> = entries
        .iter()
        .enumerate()
        .filter(|(at, entry)| {
            !entry.flags.contains(HitFlags::BLOCKER)
                && entry.id != anchor
                && !inside_anchor(*at as u32)
        })
        .map(|(_, entry)| entry)
        .collect();
    assert!(!overlay.is_empty(), "the overlay's rows are in the array");
    for entry in overlay {
        assert!(entry.x1 > entry.x0 && entry.y1 > entry.y0, "{entry:?}");
        // Seated below the invoker, which was laid out at the origin with a 24-DIP row.
        assert!(entry.y0 >= 24.0, "placed under its anchor: {entry:?}");
    }
}

#[test]
fn a_light_dismiss_press_closes_it_and_restores_focus_to_the_invoker() {
    // The router consumes the press before this layer sees it, so what is asserted is that
    // the right overlay closes and focus returns to the invoker rather than to the window.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    focus.seed(anchor);

    let _open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);
    let blocker = entries(&patch)
        .into_iter()
        .find(|entry| entry.flags.contains(HitFlags::BLOCKER))
        .expect("a light overlay contributes one")
        .id;

    overlays.service(
        &[Report::Dismiss {
            blocker,
            scope: None,
        }],
        &[],
        &mut focus.ops,
    );
    assert!(overlays.is_empty());
    assert_eq!(popped(&focus.ops).len(), 1, "one scope was closed");
    focus.sync(&patch);
    assert_eq!(
        focus.current(),
        Some(anchor),
        "focus went back to the invoker"
    );
    assert_eq!(focus.depth(), 0, "and the scope went with it");
    assert!(
        focus.reports.is_empty(),
        "focus was already on the invoker, so nothing moved and nothing repaints"
    );

    flush(&mut patch);
    assert!(
        entries(&patch)
            .iter()
            .all(|entry| !entry.flags.contains(HitFlags::BLOCKER)),
        "a closed overlay leaves nothing in the array"
    );
}

#[test]
fn escape_closes_a_flyout_and_a_popup_alike() {
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    focus.seed(anchor);

    for spec in [Spec::flyout(anchor), Spec::popup()] {
        let _open = overlays.open(spec, &mut focus.ops, body);
        flush(&mut patch);
        assert_eq!(overlays.depth(), 1);
        overlays.service(&[Report::Escape { scope: None }], &[], &mut focus.ops);
        assert!(overlays.is_empty(), "{:?} survived Esc", spec.kind());
        flush(&mut patch);
    }
}

#[test]
fn a_modal_is_not_light_dismissed_but_still_blocks() {
    // A press outside a modal must neither reach what the modal covers nor close it, which
    // is a blocker whose press does nothing.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::popup(), &mut focus.ops, body);
    flush(&mut patch);
    let blocker = entries(&patch)
        .into_iter()
        .find(|entry| entry.flags.contains(HitFlags::BLOCKER))
        .expect("a modal blocks the pointer too")
        .id;

    overlays.service(
        &[Report::Dismiss {
            blocker,
            scope: None,
        }],
        &[],
        &mut focus.ops,
    );
    assert_eq!(overlays.depth(), 1, "a stray press closed a modal");
    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn a_popup_traps_tab_and_a_flyout_lets_go() {
    // Against the ring the overlay pushed rather than a hand-built one, so what is checked
    // is that opening declares the right scope.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);

    for (spec, trapped) in [(Spec::popup(), true), (Spec::flyout(anchor), false)] {
        let mut focus = Ring::default();
        let mut overlays = Overlays::new();
        focus.seed(anchor);
        let _open = overlays.open(spec, &mut focus.ops, body);
        flush(&mut patch);
        focus.sync(&patch);
        let table = hits(&patch);

        // Two rows inside, then off the end.
        assert!(matches!(focus.ring.step(&table, true), Move::To { .. }));
        assert!(matches!(focus.ring.step(&table, true), Move::To { .. }));
        let off_the_end = focus.ring.step(&table, true);
        if trapped {
            assert!(
                matches!(off_the_end, Move::To { .. }),
                "a popup let focus out"
            );
        } else {
            assert_eq!(
                off_the_end,
                Move::Left,
                "tabbing past a flyout's last item should dismiss it"
            );
        }
        close_all(&mut overlays, &mut focus);
        flush(&mut patch);
    }
}

#[test]
fn closing_an_overlay_takes_everything_opened_above_it() {
    // A submenu cannot outlive the menu that anchored it, so the stack truncates rather
    // than removing one entry.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    focus.seed(anchor);

    let menu = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    let _submenu = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);
    focus.sync(&patch);
    assert_eq!(overlays.depth(), 2);
    assert_eq!(focus.depth(), 2);

    overlays.close(menu, &mut focus.ops);
    assert!(overlays.is_empty(), "a submenu outlived its menu");
    assert_eq!(
        popped(&focus.ops).len(),
        2,
        "one pop per scope, innermost first"
    );
    focus.sync(&patch);
    assert_eq!(focus.depth(), 0);
    assert_eq!(
        focus.current(),
        Some(anchor),
        "the outermost close is the one whose restore survives"
    );
    flush(&mut patch);
}

#[test]
fn a_stale_close_is_a_miss() {
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let first = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    overlays.close(first, &mut focus.ops);
    let second = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);

    // The second occupies the depth the first did. Closing the first must not close it.
    overlays.close(first, &mut focus.ops);
    assert_eq!(overlays.depth(), 1, "a stale id closed a live overlay");
    overlays.close(second, &mut focus.ops);
    assert!(overlays.is_empty());
    flush(&mut patch);
}

#[test]
fn a_thousand_opens_leak_no_slot_root_and_no_signal() {
    // A slot root is parentless and so invisible to a parent walk: the live-node and
    // hit-entry counts must return to exactly where they started.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let mut baseline = None;

    for round in 0..1_000 {
        let open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
        flush(&mut patch);
        overlays.close(open, &mut focus.ops);
        flush(&mut patch);
        focus.sync(&patch);

        // From the second round, so the first one's warm-up is not the baseline.
        if round == 1 {
            baseline = Some((live_nodes(), entries(&patch).len()));
        }
    }
    let (nodes, count) = baseline.expect("measured");
    assert_eq!(
        live_nodes(),
        nodes,
        "a signal outlived the overlay that made it"
    );
    assert_eq!(
        entries(&patch).len(),
        count,
        "an entry outlived the overlay that declared it"
    );
    assert!(
        entries(&patch)
            .iter()
            .all(|entry| !entry.flags.contains(HitFlags::BLOCKER)),
        "a slot root is still open"
    );
    assert_eq!(focus.depth(), 0);
}

#[test]
fn a_second_tap_on_the_invoker_closes_what_it_opened() {
    // A picker's own button shuts it rather than opening a second overlay behind the first.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let mount = mount(
        El::<Any>::seed(Preset::Bare)
            .control()
            .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, UiaRole::ComboBox)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH)
            .flyout(body)
            .row(text("Pick")),
        root(),
    );
    flush(&mut patch);
    let target = entries(&patch)[0].id;

    let tap = [Intent {
        target,
        what: What::Tapped,
    }];
    overlays.service(&[], &tap, &mut focus.ops);
    assert_eq!(overlays.depth(), 1, "the declared flyout opened");
    flush(&mut patch);

    overlays.service(&[], &tap, &mut focus.ops);
    assert!(overlays.is_empty(), "a second tap should close it");
    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_kind_that_takes_focus_takes_the_pointer_with_it() {
    // A modal that let a press through would let content the keyboard cannot reach be
    // edited, and a focus scope with no blocker has no first entry to be named by.
    assert!(Kind::Flyout.takes_focus().is_some());
    assert!(Kind::Popup.takes_focus().is_some());
    assert!(Kind::Tooltip.takes_focus().is_none());

    // What the blocker's press does is a separate question.
    assert!(Kind::Flyout.dismiss().light);
    assert!(
        !Kind::Popup.dismiss().light,
        "a modal is not light-dismissed"
    );
    assert_eq!(Kind::Popup.takes_focus(), Some(true), "and it traps");
}

#[test]
fn every_overlay_that_pushes_a_scope_has_a_blocker_to_name_it_by() {
    // A `FocusScope` is named by its own first entry in the array, and `FocusRing::collect`
    // falls back to index 0 when it cannot find that entry. Index 0 is the top of the
    // window's own content, so a scope with nothing to name it lets `Tab` walk the whole
    // window instead of failing.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);

    for spec in [Spec::flyout(anchor), Spec::popup()] {
        let mut focus = Ring::default();
        let mut overlays = Overlays::new();
        let _open = overlays.open(spec, &mut focus.ops, body);
        flush(&mut patch);
        focus.sync(&patch);

        assert_eq!(focus.depth(), 1, "{:?} pushed no scope", spec.kind());
        let named = focus.ring.innermost().expect("a scope is open").1.from;
        let entry = entries(&patch)
            .into_iter()
            .find(|entry| entry.id == named)
            .unwrap_or_else(|| {
                panic!("{:?}'s scope names an entry that is not there", spec.kind())
            });
        assert!(
            entry.flags.contains(HitFlags::BLOCKER),
            "a scope must begin at its blocker: {entry:?}"
        );
        close_all(&mut overlays, &mut focus);
        flush(&mut patch);
    }
}

#[test]
fn escape_closes_a_tooltip_even_though_it_pushes_no_scope() {
    // The router raises `Report::Escape` only while a focus scope is open, and a tooltip
    // pushes none, so this layer reads the raw keystroke itself. Without that arm a
    // description cannot be dismissed from the keyboard at all.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let mount = mount(
        El::<Any>::seed(Preset::Bare)
            .control()
            .hit(HitFlags::INTERACTIVE, UiaRole::Button)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH)
            .tip("Undo (Ctrl+Z)")
            .row(text("Undo")),
        root(),
    );
    flush(&mut patch);
    let target = entries(&patch)[0].id;

    // Hover, elapse the delay by hand, and the description is on screen.
    overlays.service(
        &[Report::HoverChanged {
            from: None,
            to: Some(target),
            at: windows_scene::Point { x: 10.0, y: 10.0 },
            qpc: 0,
        }],
        &[],
        &mut focus.ops,
    );
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
    assert_eq!(overlays.depth(), 1, "the tooltip did not open");
    focus.sync(&patch);
    assert_eq!(focus.depth(), 0, "a tooltip pushes no scope");
    flush(&mut patch);

    let reports = [Report::Key {
        target: None,
        event: escape(),
    }];
    let mut intents = Vec::new();
    overlays.keys(&reports, &mut focus.ops, &mut intents);
    assert!(overlays.is_empty(), "Esc left the tooltip on screen");

    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_hover_starts_one_delay_and_leaving_cancels_it() {
    // The delay is a deadline on the frame clock rather than a timer, so what is checked is
    // that exactly one opens per dwell and that leaving cancels it.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let mount = mount(
        El::<Any>::seed(Preset::Bare)
            .control()
            .hit(HitFlags::INTERACTIVE, UiaRole::Button)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH)
            .tip("Mute this processor")
            .row(text("Mute")),
        root(),
    );
    flush(&mut patch);
    let target = entries(&patch)[0].id;

    overlays.service(&[hover(Some(target))], &[], &mut focus.ops);
    assert_eq!(delay_ops(&mut patch), (1, 0), "one delay opened");

    // Still on the same target: not a second delay, or a tooltip would never appear while
    // the pointer jitters inside one control.
    overlays.service(&[hover(Some(target))], &[], &mut focus.ops);
    assert_eq!(delay_ops(&mut patch), (0, 0));

    overlays.service(&[hover(None)], &[], &mut focus.ops);
    assert_eq!(delay_ops(&mut patch), (0, 1), "leaving cancelled it");

    drop(mount);
    flush(&mut patch);
}

/// Returns an `Esc` key-down event as the router delivers it.
fn escape() -> KeyEvent {
    KeyEvent {
        kind: KeyKind::Down,
        key: VK_ESCAPE as u16,
        mods: Mods::default(),
        repeat: false,
    }
}

/// Flushes and returns how many delays that patch started and how many it cancelled.
fn delay_ops(patch: &mut SinkPatch) -> (usize, usize) {
    flush(patch);
    let started = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, windows_scene::Op::Delay { .. }))
        .count();
    let cancelled = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, windows_scene::Op::CancelDelay { .. }))
        .count();
    (started, cancelled)
}

/// Flushes and returns the delay id that patch opened. Panics where none was opened.
fn pending_delay(patch: &mut SinkPatch) -> windows_scene::DelayId {
    flush(patch);
    patch
        .ops()
        .iter()
        .find_map(|op| match op {
            windows_scene::Op::Delay { id, .. } => Some(*id),
            _ => None,
        })
        .expect("a hover over a described control opens one")
}

/// Returns a hover crossing as the router publishes them. Several reach one service call
/// when the pointer batch crossed several targets.
fn hover(to: Option<ControlId>) -> Report {
    Report::HoverChanged {
        from: None,
        to,
        at: windows_scene::Point { x: 10.0, y: 10.0 },
        qpc: 0,
    }
}

/// Returns a press as the router delivers it. This layer reads only the target and the fact
/// of the press, so the sample's contents do not matter.
fn press(target: ControlId) -> Report {
    let at = windows_scene::Point { x: 10.0, y: 10.0 };
    Report::Pressed {
        target,
        contact: 1,
        sample: crate::input::Sample {
            id: 1,
            ptype: crate::input::PointerType::Mouse,
            flags: crate::input::PointerFlags::default(),
            at,
            raw: at,
            contact: (0.0, 0.0),
            pen: None,
            time: 0,
            qpc: 0,
        },
        buttons: 1,
    }
}

/// Mounts three described controls and returns the mount with their ids in array order.
fn strip(patch: &mut SinkPatch, tips: [crate::widget::TextSource; 3]) -> (Mount, Vec<ControlId>) {
    let [a, b, c] = tips;
    let mount = mount(
        El::<Any>::seed(Preset::Bare).stack((
            button("A").tip(a),
            button("B").tip(b),
            button("C").tip(c),
        )),
        root(),
    );
    flush(patch);
    let ids: Vec<ControlId> = entries(patch).iter().map(|entry| entry.id).collect();
    assert_eq!(ids.len(), 3, "three described controls: {ids:?}");
    (mount, ids)
}

#[test]
fn a_description_opens_on_the_side_the_author_named() {
    // `place` flips and clamps against the window and is handed one box, so it never sees
    // the controls beside the anchor and cannot pick the side: below clears the neighbours
    // of a toolbar button and covers the next item of a vertical rail. The author states
    // the side, and this checks the statement reaches the spec the overlay opens with.
    let probe = |side: Option<Side>| {
        let mut patch = fixture();
        let mut focus = Ring::default();
        let mut overlays = Overlays::new();
        let described = El::<Any>::seed(Preset::Bare)
            .control()
            .hit(HitFlags::INTERACTIVE, UiaRole::Button)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH);
        let mount = mount(
            match side {
                Some(side) => described.tip_at(side, "Mute"),
                None => described.tip("Mute"),
            }
            .row(text("Mute")),
            root(),
        );
        flush(&mut patch);
        let target = entries(&patch)[0];

        overlays.service(&[hover(Some(target.id))], &[], &mut focus.ops);
        let delay = pending_delay(&mut patch);
        overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
        flush(&mut patch);
        let root = overlays
            .open
            .last()
            .expect("the description did not open")
            .root;
        let tip = Host::with(|host| host.model().solved(root.node()).rect);

        drop(mount);
        flush(&mut patch);
        (target, tip)
    };

    let (target, tip) = probe(None);
    assert!(
        tip.y0 >= target.y1,
        "the default description is not below its control: {tip:?} against {target:?}"
    );

    let (target, tip) = probe(Some(Side::Right));
    assert!(
        tip.x0 >= target.x1,
        "a description asked for the trailing side did not go there: {tip:?}"
    );
    assert!(
        tip.y0 < target.y1,
        "it went beside *and* below, so it still covers whatever follows the control"
    );
}

#[test]
fn one_batch_of_crossings_arms_one_delay() {
    // The pointer layer publishes every crossing in a sample batch, and the closing half
    // needs all of them: a target crossed and left between two samples is a real enter and a
    // real leave. Revealing needs only the last, because answering per crossing would arm
    // and tear down a delay for every control the sweep passed through.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(
        &[
            hover(Some(ids[0])),
            hover(Some(ids[1])),
            hover(Some(ids[2])),
        ],
        &[],
        &mut focus.ops,
    );
    assert_eq!(
        delay_ops(&mut patch),
        (1, 0),
        "one delay for the target the sweep ended on, and nothing to cancel"
    );

    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_dwell_survives_leaving_and_returning_within_one_batch() {
    // The dwell is answered at the batch's end rather than at each crossing, so an excursion
    // off a control and back inside one tick does not restart the wait.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(&[hover(Some(ids[0]))], &[], &mut focus.ops);
    assert_eq!(delay_ops(&mut patch), (1, 0));

    overlays.service(
        &[hover(Some(ids[1])), hover(Some(ids[0]))],
        &[],
        &mut focus.ops,
    );
    assert_eq!(
        delay_ops(&mut patch),
        (0, 0),
        "the wait was restarted by an excursion the user could not have made deliberately"
    );

    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_press_in_the_same_batch_as_the_hover_reveals_nothing() {
    // A description is owed to a pointer at rest, and reaching a button and pressing it
    // inside one tick is one gesture. Any press is the single exit, including for a reveal
    // this tick's crossings had not performed yet.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(&[hover(Some(ids[0])), press(ids[0])], &[], &mut focus.ops);
    assert_eq!(delay_ops(&mut patch), (0, 0), "a press armed a description");

    drop(mount);
    flush(&mut patch);
}

#[test]
fn passing_through_a_described_control_never_reads_its_text() {
    // A description's text is read once as it opens, and that read runs application code.
    // Answering a sweep per crossing would mount and destroy a tooltip for every control
    // passed through, each holding the frame clock for its exit, for content never on screen
    // for a frame. The read count is one per description actually shown.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let reads = std::rc::Rc::new(std::cell::Cell::new(0_u32));
    let counted = || {
        let reads = std::rc::Rc::clone(&reads);
        crate::widget::reactive(move |out| {
            reads.set(reads.get() + 1);
            out.push_str("described");
        })
    };
    let (mount, ids) = strip(&mut patch, [counted(), counted(), counted()]);

    // One description on screen, the ordinary way.
    overlays.service(&[hover(Some(ids[0]))], &[], &mut focus.ops);
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 1, "the description is up");
    assert_eq!(reads.get(), 1, "read once as it opened");

    // Now sweep across the rest of the strip in one batch.
    overlays.service(
        &[hover(Some(ids[1])), hover(Some(ids[2]))],
        &[],
        &mut focus.ops,
    );
    flush(&mut patch);
    assert_eq!(
        reads.get(),
        2,
        "the control the pointer merely passed through had its text read"
    );
    assert_eq!(overlays.depth(), 1, "and exactly one description is up");
    // Swapped rather than re-delayed, so no new wait is started.
    assert_eq!(delay_ops(&mut patch), (0, 0), "the swap started a new wait");

    close_all(&mut overlays, &mut focus);
    drop(mount);
    flush(&mut patch);
}

#[test]
fn hovering_a_row_that_expands_opens_it_and_leaving_for_a_sibling_closes_it() {
    // The delay's second consumer. A hover-opened submenu closes when the pointer reaches a
    // sibling row, and a clicked flyout does not close when the pointer goes anywhere; only
    // `by_dwell` separates the two.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    // A menu of two expandable rows, opened the ordinary way.
    let menu_body = || {
        flyout().stack((
            button("Alpha").flyout(body).name("Alpha"),
            button("Beta").flyout(body).name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus.ops, menu_body);
    flush(&mut patch);

    let rows: Vec<ControlId> = entries(&patch)
        .iter()
        .filter(|entry| !entry.flags.contains(HitFlags::BLOCKER))
        .map(|entry| entry.id)
        .collect();
    assert!(rows.len() >= 2, "two rows: {rows:?}");
    let (alpha, beta) = (rows[0], rows[1]);

    // Rest on the first row: a delay opens, and elapsing it opens the nested list.
    overlays.service(&[hover(Some(alpha))], &[], &mut focus.ops);
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
    assert_eq!(overlays.depth(), 2, "the submenu did not open");
    flush(&mut patch);

    // Moving to a sibling row of the parent closes the submenu. The sibling sits in the
    // parent menu, which makes the crossing a leave rather than a move into the submenu.
    overlays.service(&[hover(Some(beta))], &[], &mut focus.ops);
    assert_eq!(
        overlays.depth(),
        1,
        "a hover-opened submenu outlived its row"
    );
    flush(&mut patch);

    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn a_hover_open_closes_what_the_pointer_left_and_keeps_what_it_returned_to() {
    // Three levels, where closing every hover-opened overlay differs from closing the first
    // one. Returning from a sub-submenu to a row of the submenu takes the sub-submenu and
    // leaves the submenu; a search from the bottom of the stack finds the submenu, decides
    // it is not above the pointer, and closes nothing.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let leaf = || flyout().stack((button("Leaf1").name("Leaf1"), button("Leaf2").name("Leaf2")));
    let mid = move || {
        flyout().stack((
            button("Mid1").flyout(leaf).name("Mid1"),
            button("Mid2").flyout(leaf).name("Mid2"),
        ))
    };
    let top = move || {
        flyout().stack((
            button("Top1").flyout(mid).name("Top1"),
            button("Top2").flyout(mid).name("Top2"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus.ops, top);
    flush(&mut patch);

    let rows = |patch: &SinkPatch| -> Vec<ControlId> {
        entries(patch)
            .iter()
            .filter(|entry| !entry.flags.contains(HitFlags::BLOCKER))
            .map(|entry| entry.id)
            .collect()
    };
    let mut dwell_open = |overlays: &mut Overlays, patch: &mut SinkPatch, target| {
        overlays.service(&[hover(Some(target))], &[], &mut focus.ops);
        let delay = pending_delay(patch);
        overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
        flush(patch);
    };

    let top1 = rows(&patch)[0];
    dwell_open(&mut overlays, &mut patch, top1);
    assert_eq!(overlays.depth(), 2);

    let all = rows(&patch);
    let (mid1, mid2) = (all[2], all[3]);
    dwell_open(&mut overlays, &mut patch, mid1);
    assert_eq!(overlays.depth(), 3);

    // Back to a sibling row inside level two.
    overlays.service(&[hover(Some(mid2))], &[], &mut focus.ops);
    assert_eq!(
        overlays.depth(),
        2,
        "the level the pointer left stayed open, or the one it returned to was taken with it"
    );
    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn escape_takes_the_description_before_the_menu_under_it() {
    // One keystroke closes one overlay. The router raises `Escape` rather than a key
    // wherever a focus scope is open, so a menu with a description over it would otherwise
    // read the same press twice and lose both.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let menu_body = || {
        flyout().stack((
            button("Alpha").name("Alpha").tip("describe alpha"),
            button("Beta").name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus.ops, menu_body);
    flush(&mut patch);
    let alpha = entries(&patch)
        .iter()
        .find(|entry| !entry.flags.contains(HitFlags::BLOCKER))
        .expect("a row")
        .id;

    overlays.service(
        &[Report::HoverChanged {
            from: None,
            to: Some(alpha),
            at: windows_scene::Point { x: 10.0, y: 10.0 },
            qpc: 0,
        }],
        &[],
        &mut focus.ops,
    );
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 2, "the menu and its description");

    let escape = [Report::Escape { scope: None }];
    overlays.service(&escape, &[], &mut focus.ops);
    assert_eq!(overlays.depth(), 1, "the menu went with the description");
    flush(&mut patch);

    // And the second one reaches the menu, so nothing is stranded by the first.
    overlays.service(&escape, &[], &mut focus.ops);
    assert!(overlays.is_empty(), "a second Esc did not reach the menu");
    flush(&mut patch);
}

#[test]
fn closing_a_menu_cancels_the_dwell_it_started() {
    // A delay outliving its menu holds a frame-clock `Tick` for its full duration and then
    // opens a submenu against a row that has gone. Every close path reaches `truncate`,
    // which is where it is cancelled.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let leaf = || flyout().stack(button("Leaf").name("Leaf"));
    let menu_body = move || {
        flyout().stack((
            button("Alpha").flyout(leaf).name("Alpha"),
            button("Beta").name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus.ops, menu_body);
    flush(&mut patch);
    let alpha = entries(&patch)
        .iter()
        .find(|entry| !entry.flags.contains(HitFlags::BLOCKER))
        .expect("a row")
        .id;

    overlays.service(
        &[Report::HoverChanged {
            from: None,
            to: Some(alpha),
            at: windows_scene::Point { x: 10.0, y: 10.0 },
            qpc: 0,
        }],
        &[],
        &mut focus.ops,
    );
    let delay = pending_delay(&mut patch);

    close_all(&mut overlays, &mut focus);
    assert_eq!(
        delay_ops(&mut patch),
        (0, 1),
        "closing left the delay running"
    );

    // And the one that was in flight reports against nothing.
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus.ops);
    assert!(
        overlays.is_empty(),
        "a closed menu's dwell opened a submenu"
    );
    flush(&mut patch);
}

#[test]
fn a_clicked_flyout_does_not_close_because_the_pointer_moved() {
    // `by_dwell` is recorded rather than inferred because these two overlays are the same
    // kind, opened the same way, differing only in what opened them, and the right response
    // to the pointer leaving is opposite in each.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);

    overlays.service(
        &[Report::HoverChanged {
            from: None,
            to: None,
            at: windows_scene::Point { x: 1.0, y: 1.0 },
            qpc: 0,
        }],
        &[],
        &mut focus.ops,
    );
    assert_eq!(
        overlays.depth(),
        1,
        "a flyout the user clicked open closed because the pointer left it"
    );
    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn dropping_the_stack_releases_a_pending_delay() {
    // A pending delay outlives the stack twice over: its id stays claimed in the model, and
    // its batch keeps the frame clock awake. The drop releases both.
    let mut patch = fixture();
    let mut focus = Ring::default();

    let mount = mount(
        El::<Any>::seed(Preset::Bare)
            .control()
            .hit(HitFlags::INTERACTIVE, UiaRole::Button)
            .width(crate::role::Metric::CardMinW)
            .height(crate::role::Metric::RowH)
            .tip("Undo")
            .row(text("Undo")),
        root(),
    );
    flush(&mut patch);
    let target = entries(&patch)[0].id;

    {
        let mut overlays = Overlays::new();
        overlays.service(
            &[Report::HoverChanged {
                from: None,
                to: Some(target),
                at: windows_scene::Point { x: 10.0, y: 10.0 },
                qpc: 0,
            }],
            &[],
            &mut focus.ops,
        );
        assert_eq!(delay_ops(&mut patch), (1, 0), "a delay is pending");
    }
    assert_eq!(delay_ops(&mut patch), (0, 1), "the drop cancelled it");

    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_declared_popup_preserves_intent_on_resize_and_disposes_on_dismissal() {
    use crate::role::Metric;
    use crate::signal::{Cell, flush as flush_signals};
    let mut patch = fixture();
    let wanted = Cell::new(false);
    let narrow = Cell::new(true);
    let host = mount(
        button("Inspector")
            .popup_when(
                move || wanted.get() && narrow.get(),
                Spec::popup().exit(Exit::Fade { ms: 200 }),
                move || wanted.set(false),
                || {
                    let _local = Cell::new(1_u32);
                    body()
                },
            )
            .width(Metric::CardMinW),
        root(),
    );
    flush(&mut patch);
    let invoker = entries(&patch)[0].id;
    let baseline = live_nodes();
    let mut focus = Ring::default();
    focus.seed(invoker);
    let mut overlays = Overlays::new();
    for _ in 0..3 {
        wanted.set(true);
        flush_signals();
        overlays.sync(&mut focus.ops);
        assert_eq!(overlays.depth(), 1);
        assert!(live_nodes() > baseline);
        narrow.set(false);
        flush_signals();
        overlays.sync(&mut focus.ops);
        assert!(overlays.is_empty());
        assert!(wanted.get(), "resize must preserve narrow intent");
        assert_eq!(live_nodes(), baseline);
        narrow.set(true);
        flush_signals();
        overlays.sync(&mut focus.ops);
        assert_eq!(overlays.depth(), 1);
        patch.clear();
        overlays.close_top(&mut focus.ops);
        assert!(!wanted.get());
        flush(&mut patch);
        focus.sync(&patch);
        assert_eq!(focus.current(), Some(invoker));
        assert!(patch.ops().iter().any(|op| matches!(
            op,
            windows_scene::Op::Drop {
                exit: Exit::Fade { ms: 200 },
                ..
            }
        )));
    }
    wanted.set(true);
    flush_signals();
    overlays.sync(&mut focus.ops);
    drop(host);
    overlays.sync(&mut focus.ops);
    assert!(overlays.is_empty());
    assert!(wanted.get(), "unmount is not a user dismissal");
}

#[test]
fn a_drawer_viewport_sizes_from_window_input_and_clips_its_shadow() {
    use crate::layout::{Edge, Len, probe};
    use crate::role::Metric;
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let pane = probe();
    let id = overlays.open(
        Spec::popup().anchor(Anchor::window(Side::Right)).viewport([
            Len::Metric(Metric::RowH),
            Len::Metric(crate::role::tests::TOP_BAND),
            Len::Zero,
            Len::Metric(crate::role::tests::BOTTOM_BAND),
        ]),
        &mut focus.ops,
        || {
            crate::widget::sheet("drawer")
                .shadowed(Edge::Left)
                .width(Len::Pct(0.82))
                .max_width(crate::role::tests::MAX_WIDTH)
                .height(Len::Pct(1.0))
                .probed(pane)
        },
    );
    for width in [1100.0, 1500.0, 500.0] {
        Host::with(|h| h.set_window(Vector2 { x: width, y: 640.0 }));
        flush(&mut patch);
        let measured = pane.get();
        let (left, top, bottom) = Host::with(|h| {
            (
                crate::role::metric(Metric::RowH, h.root_scope),
                crate::role::metric(crate::role::tests::TOP_BAND, h.root_scope),
                crate::role::metric(crate::role::tests::BOTTOM_BAND, h.root_scope),
            )
        });
        assert!((measured.size.x - ((width - left) * 0.82).min(420.0)).abs() < 1.0);
        assert_eq!(measured.rect.x1, width);
        assert_eq!(measured.rect.y0, top);
        assert_eq!(measured.rect.y1, 640.0 - bottom);
        let root = overlays.open[id.depth as usize].root;
        assert!(Host::with(|h| h.model().solved(root.node()).bounded));
    }
    overlays.close(id, &mut focus.ops);
}

fn sliding_popup() -> Spec {
    Spec::popup()
        .anchor(Anchor::window(Side::Right))
        .viewport([crate::layout::Len::Zero; 4])
        .slide(
            Vector2 { x: 1.0, y: 0.0 },
            200,
            windows_scene::Easing::Cubic {
                c1: Vector2 { x: 0.16, y: 1.0 },
                c2: Vector2 { x: 0.3, y: 1.0 },
            },
        )
}

#[test]
fn a_scrolling_flyout_keeps_its_rail_above_the_choices() {
    use crate::layout::{Len, Reveal, scroll_with, stack};
    use crate::role::Metric;
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let id = overlays.open(Spec::flyout(anchor), &mut focus.ops, || {
        flyout().stack(
            scroll_with(
                Reveal::Always,
                stack((button("Alpha"), button("Beta"), button("Gamma"), button("Delta"))),
            )
            .height(Len::Times(Metric::RowH, 2.0))
            .width(Len::Times(Metric::RowH, 11.0)),
        )
    });
    flush(&mut patch);
    let (grab, rect) = Host::with(|h| {
        let row = h.scrolls.iter().next().unwrap().1;
        (row.grab.unwrap(), h.model.solved(row.rail.unwrap().node()).rect)
    });
    let table = hits(&patch);
    let hit = table
        .hit(
            Vector2 { x: (rect.x0 + rect.x1) * 0.5, y: (rect.y0 + rect.y1) * 0.5 },
            windows_scene::ContactKind::Mouse,
        )
        .unwrap();
    assert_eq!(hit.id, grab, "rail must win over choices after overlay placement");
    overlays.close(id, &mut focus.ops);
}

#[test]
fn a_slide_waits_for_the_compositor_and_never_emits_per_frame_updates() {
    use crate::layout::{Len, scroll, stack};
    use windows_scene::{Anim, Bind, Iterations, Op, Prop};
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    focus.seed(anchor);
    let mut overlays = Overlays::new();
    let id = overlays.open(sliding_popup(), &mut focus.ops, || {
        scroll(
            stack((button("Alpha"), button("Beta")))
                .height(Len::Times(crate::role::Metric::RowH, 37.5)),
        )
        .width(Len::Times(crate::role::Metric::RowH, 12.5))
        .height(Len::Pct(1.0))
    });
    let node = overlays.open[id.depth as usize]
        .mount
        .as_ref()
        .unwrap()
        .node();
    flush(&mut patch);
    let animation = patch
        .ops()
        .iter()
        .find_map(|op| match *op {
            Op::Bind {
                id,
                prop: Prop::Offset,
                bind:
                    Bind::Animate(Anim::Frames {
                        frames,
                        duration_ms,
                        iterations,
                    }),
            } if id == node => Some((frames, duration_ms, iterations)),
            _ => None,
        })
        .expect("one group enters");
    assert_eq!((animation.1, animation.2), (200, Iterations::Count(1)));
    assert_eq!(
        entries(&patch)
            .iter()
            .filter(|e| e.flags.contains(HitFlags::BLOCKER))
            .count(),
        1
    );
    assert!(
        !patch.ops().iter().any(|op| matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Create { .. },
                ..
            }
        )),
        "a moving viewport cannot own a live gesture source"
    );
    assert!(Host::with(|h| h.model.input_suspended(node)));
    for _ in 0..5 {
        patch.clear();
        flush(&mut patch);
        assert!(patch.ops().is_empty(), "entry must advance entirely in DWM");
    }
    overlays.scene(
        &[SceneEvent::AnimationCompleted {
            node,
            prop: Prop::Offset,
        }],
        &mut focus.ops,
    );
    flush(&mut patch);
    assert!(!Host::with(|h| h.model.input_suspended(node)));
    assert!(
        patch.ops().iter().any(|op| matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Create { .. },
                ..
            }
        )),
        "scroll source activates after entry"
    );
    assert!(
        patch.ops().iter().any(|op| matches!(op,
        Op::Tracker { op: windows_scene::TrackerOp::Bounds { max, .. }, .. } if max.y > 0.0)),
        "deferred creation must publish bounds too, or the new tracker stays at zero"
    );
    focus.sync(&patch);
    let table = hits(&patch);
    assert!(
        matches!(focus.ring.step(&table, true), Move::To { .. }),
        "keyboard enters the popup"
    );
    patch.clear();
    overlays.close(id, &mut focus.ops);
    flush(&mut patch);
    focus.sync(&patch);
    assert_eq!(focus.current(), Some(anchor));
    assert!(patch.ops().iter().any(|op| matches!(
        op,
        Op::Drop {
            exit: Exit::Slide { ms: 200, .. },
            ..
        }
    )));
    assert!(
        !entries(&patch)
            .iter()
            .any(|e| e.flags.contains(HitFlags::BLOCKER)),
        "exit pixels own no input"
    );
}

#[test]
fn resizing_an_entering_popup_snaps_and_a_stale_completion_cannot_release_a_reopen() {
    use crate::layout::Len;
    use windows_scene::{Bind, Op, Prop};
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    let open = |overlays: &mut Overlays, focus: &mut Ring| {
        overlays.open(sliding_popup(), &mut focus.ops, || {
            body()
                .width(Len::Times(crate::role::Metric::RowH, 12.5))
                .height(Len::Pct(1.0))
        })
    };
    let id = open(&mut overlays, &mut focus);
    let node = overlays.open[id.depth as usize]
        .mount
        .as_ref()
        .unwrap()
        .node();
    flush(&mut patch);
    patch.clear();
    // Height-only resize changes no horizontal local offset. It must still stop entry.
    Host::with(|h| h.set_window(Vector2 { x: 800.0, y: 640.0 }));
    flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Bind { id, prop: Prop::Offset, bind: Bind::Set(_) } if *id == node)));
    assert!(!Host::with(|h| h.model.input_suspended(node)));
    overlays.close(id, &mut focus.ops);
    let second = open(&mut overlays, &mut focus);
    let fresh = overlays.open[second.depth as usize]
        .mount
        .as_ref()
        .unwrap()
        .node();
    flush(&mut patch);
    assert_ne!(node, fresh);
    overlays.scene(
        &[SceneEvent::AnimationCompleted {
            node,
            prop: Prop::Offset,
        }],
        &mut focus.ops,
    );
    assert!(Host::with(|h| h.model.input_suspended(fresh)));
    patch.clear();
    overlays.close(second, &mut focus.ops);
    flush(&mut patch);
    assert!(
        !patch.ops().iter().any(|op| matches!(
            op,
            Op::Drop {
                exit: Exit::Slide { .. },
                ..
            }
        )),
        "an interrupted entry closes immediately instead of jumping to a resting snapshot"
    );
}

#[test]
fn a_window_presentation_switch_snaps_both_ways_and_keeps_the_next_dismissal_animated() {
    use crate::signal::{Cell, flush as flush_signals};
    use windows_scene::{Bind, Op};
    let mut patch = fixture();
    let window = Host::window_size();
    let wanted = Cell::new(true);
    let (_owner, _mount) = Owner::scope(|| {
        mount(
            button("Inspector").popup_when(
                move || window.get().x < 1000.0 && wanted.get(),
                sliding_popup(),
                move || wanted.set(false),
                body,
            ),
            root(),
        )
    });
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    overlays.sync(&mut focus.ops);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 1);
    let node = overlays.open[0].mount.as_ref().unwrap().node();
    overlays.scene(
        &[SceneEvent::AnimationCompleted {
            node,
            prop: windows_scene::Prop::Offset,
        }],
        &mut focus.ops,
    );
    patch.clear();
    Host::with(|h| {
        h.set_window(Vector2 {
            x: 1200.0,
            y: 600.0,
        })
    });
    flush_signals();
    overlays.sync(&mut focus.ops);
    flush(&mut patch);
    assert!(overlays.is_empty());
    assert!(wanted.get());
    assert!(!patch.ops().iter().any(|op| matches!(
        op,
        Op::Drop {
            exit: Exit::Slide { .. },
            ..
        }
    )));
    patch.clear();
    Host::with(|h| h.set_window(Vector2 { x: 800.0, y: 600.0 }));
    flush_signals();
    overlays.sync(&mut focus.ops);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 1);
    assert!(!patch.ops().iter().any(|op| matches!(
        op,
        Op::Bind {
            bind: Bind::Animate(_),
            ..
        }
    )));
    patch.clear();
    wanted.set(false);
    flush_signals();
    overlays.sync(&mut focus.ops);
    flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(
        op,
        Op::Drop {
            exit: Exit::Slide { ms: 200, .. },
            ..
        }
    )));
}

#[test]
fn opening_emits_one_scope_named_by_its_blocker_and_restoring_to_the_invoker() {
    // The overlay layer reaches no ring. What it opens is one op, and the invoker is on it:
    // the half holding the ring cannot know which control a flyout was opened from.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);

    let opened = pushed(&focus.ops);
    assert_eq!(opened.len(), 1, "{:?}", focus.ops);
    let (_, trap, restore_to) = opened[0];
    assert!(!trap, "a flyout lets Tab leave");
    assert_eq!(restore_to, Some(anchor));
    let FocusOp::PushScope { from, .. } = focus.ops[0] else {
        panic!("the first op opens the scope: {:?}", focus.ops)
    };
    let entry = entries(&patch)
        .into_iter()
        .find(|entry| entry.id == from)
        .expect("the scope names an entry that is in the array");
    assert!(
        entry.flags.contains(HitFlags::BLOCKER),
        "a scope must begin at its blocker: {entry:?}"
    );

    // An overlay anchored to the window names no target, and the ring fills in the focus it
    // interrupted.
    focus.ops.clear();
    let _modal = overlays.open(Spec::popup(), &mut focus.ops, body);
    assert_eq!(pushed(&focus.ops), [(ScopeId(2), true, None)]);
    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn closing_the_top_pops_exactly_the_scopes_it_closed_in_order() {
    // The pops are innermost first, which is the order the ring applies them in: the
    // outermost pop is the last, so its restore target is where focus ends.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _menu = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    let _submenu = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    let opened: Vec<ScopeId> = pushed(&focus.ops).into_iter().map(|(id, ..)| id).collect();
    assert_eq!(opened.len(), 2);
    focus.ops.clear();

    overlays.close_top(&mut focus.ops);
    assert_eq!(popped(&focus.ops), [opened[1]], "only the submenu closed");
    focus.ops.clear();

    overlays.close_top(&mut focus.ops);
    assert_eq!(popped(&focus.ops), [opened[0]]);
    flush(&mut patch);
}

#[test]
fn type_ahead_walks_the_items_of_the_topmost_overlay_and_cycles() {
    // Resolved against the array the last solve built, filtered to what can take focus, and
    // cycled from the item this overlay last typed onto. The focused control is the input
    // half's, so the cycle cannot be relative to it.
    let mut patch = fixture();
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let menu_body = || {
        flyout().stack((
            button("Alpha").name("Alpha"),
            button("Almond").name("Almond"),
            button("Beta").name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus.ops, menu_body);
    flush(&mut patch);
    let rows: Vec<ControlId> = entries(&patch)
        .iter()
        .filter(|entry| !entry.flags.contains(HitFlags::BLOCKER))
        .map(|entry| entry.id)
        .collect();
    assert!(rows.len() >= 3, "three rows: {rows:?}");
    focus.ops.clear();

    let mut intents = Vec::new();
    let letter = |key: char| {
        vec![Report::Key {
            target: None,
            event: KeyEvent {
                kind: KeyKind::Down,
                key: key as u16,
                mods: Mods::default(),
                repeat: false,
            },
        }]
    };

    overlays.keys(&letter('A'), &mut focus.ops, &mut intents);
    assert_eq!(focus.ops, [FocusOp::Focus(Some(rows[0]))]);
    focus.ops.clear();

    // The same letter again cycles to the next item beginning with it, rather than sticking.
    overlays.keys(&letter('A'), &mut focus.ops, &mut intents);
    assert_eq!(focus.ops, [FocusOp::Focus(Some(rows[1]))]);
    focus.ops.clear();

    overlays.keys(&letter('B'), &mut focus.ops, &mut intents);
    assert_eq!(focus.ops, [FocusOp::Focus(Some(rows[2]))]);
    focus.ops.clear();

    // A letter no item answers moves nothing.
    overlays.keys(&letter('Z'), &mut focus.ops, &mut intents);
    assert!(focus.ops.is_empty());
    assert!(intents.is_empty(), "type-ahead invokes nothing");

    close_all(&mut overlays, &mut focus);
    flush(&mut patch);
}

#[test]
fn the_arrow_keys_emit_the_step_they_mean_and_left_closes_a_level() {
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();

    let _menu = overlays.open(Spec::flyout(anchor), &mut focus.ops, body);
    flush(&mut patch);
    focus.ops.clear();

    let mut intents = Vec::new();
    let key = |code: i32| {
        vec![Report::Key {
            target: None,
            event: KeyEvent {
                kind: KeyKind::Down,
                key: code as u16,
                mods: Mods::default(),
                repeat: false,
            },
        }]
    };
    overlays.keys(&key(VK_DOWN), &mut focus.ops, &mut intents);
    overlays.keys(&key(VK_UP), &mut focus.ops, &mut intents);
    overlays.keys(&key(VK_HOME), &mut focus.ops, &mut intents);
    overlays.keys(&key(VK_END), &mut focus.ops, &mut intents);
    assert_eq!(
        focus.ops,
        [
            FocusOp::Step { forward: true },
            FocusOp::Step { forward: false },
            FocusOp::StepToEnd { last: false },
            FocusOp::StepToEnd { last: true },
        ]
    );

    // And the ring moves where those steps point, which is the other half of the seam.
    focus.sync(&patch);
    assert!(focus.current().is_some(), "the steps reached the ring");
    assert!(
        !focus.reports.is_empty(),
        "each move is reported, so the ring visual repaints"
    );

    overlays.keys(&key(VK_LEFT), &mut focus.ops, &mut intents);
    assert!(overlays.is_empty(), "Left closes one level");
    assert_eq!(popped(&focus.ops).len(), 1);
    flush(&mut patch);
}

#[test]
fn the_ring_restores_on_a_pop_and_records_the_focus_a_scope_interrupted() {
    // The ring's half of the seam, driven by ops alone. A pop restores and reports, and a
    // scope naming no target takes whatever had focus when it opened.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = Ring::default();
    let mut overlays = Overlays::new();
    focus.seed(anchor);

    let _menu = overlays.open(Spec::popup(), &mut focus.ops, body);
    flush(&mut patch);
    focus.sync(&patch);
    assert_eq!(
        focus
            .ring
            .innermost()
            .expect("a scope is open")
            .1
            .restore_to,
        Some(anchor),
        "a window-anchored scope records the focus it interrupted"
    );

    // Tab into the overlay, then close it.
    let table = hits(&patch);
    assert!(matches!(focus.ring.step(&table, true), Move::To { .. }));
    let inside = focus.current().expect("focus is on a row");
    assert_ne!(inside, anchor);

    overlays.close_top(&mut focus.ops);
    focus.sync(&patch);
    assert_eq!(focus.current(), Some(anchor));
    assert_eq!(
        focus.reports,
        [Report::FocusChanged {
            from: Some(inside),
            to: Some(anchor),
        }],
        "the restore is reported, so both rings repaint"
    );
    flush(&mut patch);
}
