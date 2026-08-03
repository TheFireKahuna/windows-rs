//! What the overlay layer claims, checked by driving it headless.
//!
//! `Model` owns no COM, so a whole open-and-dismiss runs with no window, no device and no
//! compositor: the slot roots, the blocker entry, the focus scope and the placement all read
//! back off the patch. What needs no input is asserted here; what needs a real press against
//! a real window is a harness a person runs.

use super::*;
use crate::build::{Any, El, mount};
use crate::input::{FocusRing, KeyEvent, Mods};
use crate::layout::Preset;
use crate::signal::live_nodes;
use crate::widget::{UiaRole, button, flyout, text};
use windows_scene::{HitEntry, HitFlags, HitTable, SinkPatch};

/// The build module's own fixture: the palette, a shaper, a fresh host, first flush drained.
fn fixture() -> SinkPatch {
    crate::build::tests::fixture()
}

fn flush(patch: &mut SinkPatch) {
    Host::with(|host| host.flush(patch));
}

fn root() -> GroupId {
    Host::with(|host| host.model().root())
}

/// The hit table as the front thread would hold it, rebuilt from the patch.
fn hits(patch: &SinkPatch) -> HitTable {
    let mut table = HitTable::default();
    table.replace(patch.hit_entries());
    table
}

fn entries(patch: &SinkPatch) -> Vec<HitEntry> {
    patch.hit_entries().to_vec()
}

/// A menu-ish body: a flyout surface with two focusable rows.
fn body() -> View {
    flyout().stack((button("Alpha"), button("Beta")))
}

/// A control to anchor against, and the id it minted.
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
    // The whole of "press outside dismisses", and it is not a mechanism: the array is the
    // z-order, the scan takes the first hit from the back, so a press inside resolves to
    // the overlay and a press anywhere else resolves to the blocker.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let content = entries(&patch).len();
    let _open = overlays.open(Spec::flyout(anchor), &mut focus, body);
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
    // The gap the detached solve closed. Before it, an overlay was laid out nowhere: no
    // size, no offset, and a hit entry with zero area — visibly on screen and unhittable.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::flyout(anchor), &mut focus, body);
    flush(&mut patch);

    let entries = entries(&patch);
    // Everything that is neither the blocker nor part of the **anchor's own subtree** —
    // which is not the same as "not the anchor", because a control's label is an element
    // of its own and sits inside it.
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
    // The press is consumed by the router before this layer sees it, so what is asserted
    // here is that it closes the right overlay and puts focus back where the user came
    // from — an overlay that dismissed without restoring would leave the next keystroke
    // going to the window.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    _ = focus.focus(Some(anchor));

    let _open = overlays.open(Spec::flyout(anchor), &mut focus, body);
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
        &hits(&patch),
        &mut focus,
    );
    assert!(overlays.is_empty());
    assert_eq!(
        focus.current(),
        Some(anchor),
        "focus went back to the invoker"
    );
    assert_eq!(focus.depth(), 0, "and the scope went with it");

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
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    _ = focus.focus(Some(anchor));

    for spec in [Spec::flyout(anchor), Spec::popup()] {
        let _open = overlays.open(spec, &mut focus, body);
        flush(&mut patch);
        assert_eq!(overlays.depth(), 1);
        overlays.service(
            &[Report::Escape { scope: None }],
            &[],
            &hits(&patch),
            &mut focus,
        );
        assert!(overlays.is_empty(), "{:?} survived Esc", spec.kind());
        flush(&mut patch);
    }
}

#[test]
fn a_modal_is_not_light_dismissed_but_still_blocks() {
    // The two halves of the same decision. A press outside a modal must not reach what it
    // covers *and* must not close it, which is exactly a blocker whose press does nothing.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::popup(), &mut focus, body);
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
        &hits(&patch),
        &mut focus,
    );
    assert_eq!(overlays.depth(), 1, "a stray press closed a modal");
    overlays.close_all(&mut focus);
    flush(&mut patch);
}

#[test]
fn a_popup_traps_tab_and_a_flyout_lets_go() {
    // Against the ring the overlay actually pushed, rather than a hand-built one: the point
    // is that opening declares the right scope, not that a scope behaves once declared.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);

    for (spec, trapped) in [(Spec::popup(), true), (Spec::flyout(anchor), false)] {
        let mut focus = FocusRing::default();
        let mut overlays = Overlays::new();
        _ = focus.focus(Some(anchor));
        let _open = overlays.open(spec, &mut focus, body);
        flush(&mut patch);
        let table = hits(&patch);

        // Two rows inside, then off the end.
        assert!(matches!(focus.step(&table, true), Move::To { .. }));
        assert!(matches!(focus.step(&table, true), Move::To { .. }));
        let off_the_end = focus.step(&table, true);
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
        overlays.close_all(&mut focus);
        flush(&mut patch);
    }
}

#[test]
fn closing_an_overlay_takes_everything_opened_above_it() {
    // The nesting rule, and the whole of it: a submenu cannot outlive the menu that
    // anchored it, so the stack truncates rather than removing one entry.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    _ = focus.focus(Some(anchor));

    let menu = overlays.open(Spec::flyout(anchor), &mut focus, body);
    let _submenu = overlays.open(Spec::flyout(anchor), &mut focus, body);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 2);
    assert_eq!(focus.depth(), 2);

    overlays.close(menu, &mut focus);
    assert!(overlays.is_empty(), "a submenu outlived its menu");
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
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let first = overlays.open(Spec::flyout(anchor), &mut focus, body);
    overlays.close(first, &mut focus);
    let second = overlays.open(Spec::flyout(anchor), &mut focus, body);
    flush(&mut patch);

    // The second occupies the depth the first did. Closing the first must not close it.
    overlays.close(first, &mut focus);
    assert_eq!(overlays.depth(), 1, "a stale id closed a live overlay");
    overlays.close(second, &mut focus);
    assert!(overlays.is_empty());
    flush(&mut patch);
}

#[test]
fn a_thousand_opens_leak_no_slot_root_and_no_signal() {
    // A parentless root is invisible to a parent walk, which is the shape that leaks once
    // per unmount — so the claim is not "it usually gets cleaned up", it is that the counts
    // return to exactly where they started.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    let mut baseline = None;

    for round in 0..1_000 {
        let open = overlays.open(Spec::flyout(anchor), &mut focus, body);
        flush(&mut patch);
        overlays.close(open, &mut focus);
        flush(&mut patch);

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
    // What makes a picker's own button shut it, rather than opening a second one behind
    // the first.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
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
    overlays.service(&[], &tap, &hits(&patch), &mut focus);
    assert_eq!(overlays.depth(), 1, "the declared flyout opened");
    flush(&mut patch);

    overlays.service(&[], &tap, &hits(&patch), &mut focus);
    assert!(overlays.is_empty(), "a second tap should close it");
    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_kind_that_takes_focus_takes_the_pointer_with_it() {
    // The rule the blocker's existence rests on. A modal that let a press through would
    // let the user edit content the keyboard cannot reach, and a focus scope with no
    // blocker has no first entry to be named by — so both follow from one answer.
    assert!(Kind::Flyout.takes_focus().is_some());
    assert!(Kind::Popup.takes_focus().is_some());
    assert!(Kind::Tooltip.takes_focus().is_none());

    // And what the blocker's press *does* is the separate question.
    assert!(Kind::Flyout.dismiss().light);
    assert!(
        !Kind::Popup.dismiss().light,
        "a modal is not light-dismissed"
    );
    assert_eq!(Kind::Popup.takes_focus(), Some(true), "and it traps");
}

#[test]
fn every_overlay_that_pushes_a_scope_has_a_blocker_to_name_it_by() {
    // The join the two facts rest on. A `FocusScope` is named by its own first entry in the
    // array, `FocusRing::collect` falls back to index 0 when it cannot find that entry, and
    // index 0 is the top of the window's own content — so a scope with nothing to name it
    // does not fail, it silently lets `Tab` walk the whole window. Nothing else would say
    // so, which is why this is asserted rather than left to the comment.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);

    for spec in [Spec::flyout(anchor), Spec::popup()] {
        let mut focus = FocusRing::default();
        let mut overlays = Overlays::new();
        let _open = overlays.open(spec, &mut focus, body);
        flush(&mut patch);

        assert_eq!(focus.depth(), 1, "{:?} pushed no scope", spec.kind());
        let named = focus.innermost().expect("a scope is open").1.from;
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
        overlays.close_all(&mut focus);
        flush(&mut patch);
    }
}

#[test]
fn escape_closes_a_tooltip_even_though_it_pushes_no_scope() {
    // The dismiss route that is nobody else's. The router raises `Report::Escape` only
    // while a focus scope is open, and a tooltip deliberately pushes none — so if this
    // layer does not read the keystroke itself, a description cannot be dismissed from the
    // keyboard at all and the only symptom is one that will not go away.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
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
        &hits(&patch),
        &mut focus,
    );
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
    assert_eq!(overlays.depth(), 1, "the tooltip did not open");
    assert_eq!(focus.depth(), 0, "a tooltip pushes no scope");
    flush(&mut patch);

    let mut reports = vec![Report::Key {
        target: None,
        event: escape(),
    }];
    let mut intents = Vec::new();
    overlays.keys(&mut reports, &hits(&patch), &mut focus, &mut intents);
    assert!(overlays.is_empty(), "Esc left the tooltip on screen");

    drop(mount);
    flush(&mut patch);
}

#[test]
fn a_hover_starts_one_delay_and_leaving_cancels_it() {
    // The delay is a deadline on the frame clock and not a timer, so the only thing here is
    // that exactly one is opened per dwell and that it is cancelled rather than left to
    // fire against a target the pointer has left.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
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

    let table = hits(&patch);
    overlays.service(&[hover(Some(target))], &[], &table, &mut focus);
    assert_eq!(delay_ops(&mut patch), (1, 0), "one delay opened");

    // Still on the same target: not a second delay, or a tooltip would never appear while
    // the pointer jitters inside one control.
    overlays.service(&[hover(Some(target))], &[], &table, &mut focus);
    assert_eq!(delay_ops(&mut patch), (0, 0));

    overlays.service(&[hover(None)], &[], &table, &mut focus);
    assert_eq!(delay_ops(&mut patch), (0, 1), "leaving cancelled it");

    drop(mount);
    flush(&mut patch);
}

/// `Esc`, as the router delivers it.
fn escape() -> KeyEvent {
    KeyEvent {
        kind: KeyKind::Down,
        key: VK_ESCAPE as u16,
        mods: Mods::default(),
        repeat: false,
    }
}

/// Flushes, and answers how many delays were started and cancelled in that patch.
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

/// The delay the last flush opened.
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

/// A hover crossing, as the router publishes them — several per service when the pointer
/// batch crossed several targets.
fn hover(to: Option<ControlId>) -> Report {
    Report::HoverChanged {
        from: None,
        to,
        at: windows_scene::Point { x: 10.0, y: 10.0 },
        qpc: 0,
    }
}

/// A press, as the router delivers it. Nothing in the sample is read by this layer — the
/// target and the fact of the press are the whole of what it uses.
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

/// A strip of described controls, and the ids they minted in array order.
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
fn one_batch_of_crossings_arms_one_delay() {
    // The pointer layer publishes every crossing in a sample batch, deliberately — a target
    // crossed and left between two samples is a real enter and a real leave, and the closing
    // half needs both. Revealing does not: a sweep across a strip answers per crossing would
    // arm and tear down a delay for each control passed through, at pointer sample rate
    // rather than at hover rate. Only where the pointer came to rest is owed one.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(
        &[
            hover(Some(ids[0])),
            hover(Some(ids[1])),
            hover(Some(ids[2])),
        ],
        &[],
        &hits(&patch),
        &mut focus,
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
    // The other half of answering the batch's end rather than each crossing: a sub-tick
    // excursion off a control and back is not a reason to start the wait again. It is the
    // same judgement the pointer layer makes when it declines to sample.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(&[hover(Some(ids[0]))], &[], &hits(&patch), &mut focus);
    assert_eq!(delay_ops(&mut patch), (1, 0));

    overlays.service(
        &[hover(Some(ids[1])), hover(Some(ids[0]))],
        &[],
        &hits(&patch),
        &mut focus,
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
    // Reaching a button and pressing it inside one tick is one gesture, and a description is
    // owed to a pointer at rest. Any press is the single exit, and that has to include a
    // reveal this tick's crossings had not performed yet.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();
    let (mount, ids) = strip(&mut patch, ["a".into(), "b".into(), "c".into()]);

    overlays.service(
        &[hover(Some(ids[0])), press(ids[0])],
        &[],
        &hits(&patch),
        &mut focus,
    );
    assert_eq!(delay_ops(&mut patch), (0, 0), "a press armed a description");

    drop(mount);
    flush(&mut patch);
}

#[test]
fn passing_through_a_described_control_never_reads_its_text() {
    // The expensive half, and the one that runs *application* code. A description's text is
    // read once as it opens, so answering a sweep per crossing mounts a tooltip and destroys
    // it again for every control passed through — each intermediate one leaving a ghost that
    // holds the frame clock for its exit, for content that was never on screen for a frame.
    //
    // Counting reads is what makes that visible: it is one per description actually shown.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let reads = std::rc::Rc::new(std::cell::Cell::new(0_u32));
    let counted = || {
        let reads = std::rc::Rc::clone(&reads);
        crate::widget::reactive(move || {
            reads.set(reads.get() + 1);
            "described".to_owned()
        })
    };
    let (mount, ids) = strip(&mut patch, [counted(), counted(), counted()]);

    // One description on screen, the ordinary way.
    overlays.service(&[hover(Some(ids[0]))], &[], &hits(&patch), &mut focus);
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 1, "the description is up");
    assert_eq!(reads.get(), 1, "read once as it opened");

    // Now sweep across the rest of the strip in one batch.
    overlays.service(
        &[hover(Some(ids[1])), hover(Some(ids[2]))],
        &[],
        &hits(&patch),
        &mut focus,
    );
    flush(&mut patch);
    assert_eq!(
        reads.get(),
        2,
        "the control the pointer merely passed through had its text read"
    );
    assert_eq!(overlays.depth(), 1, "and exactly one description is up");
    // Swapped rather than re-delayed, which is what makes scanning a strip tolerable.
    assert_eq!(delay_ops(&mut patch), (0, 0), "the swap started a new wait");

    overlays.close_all(&mut focus);
    drop(mount);
    flush(&mut patch);
}

#[test]
fn hovering_a_row_that_expands_opens_it_and_leaving_for_a_sibling_closes_it() {
    // The delay's second consumer, and the pair of behaviours that look contradictory until
    // you see what separates them: a hover-opened submenu closes when the pointer reaches a
    // sibling row, and a clicked flyout does not close when the pointer goes anywhere. Only
    // `by_dwell` tells them apart.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    // A menu of two expandable rows, opened the ordinary way.
    let menu_body = || {
        flyout().stack((
            button("Alpha").flyout(body).name("Alpha"),
            button("Beta").flyout(body).name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus, menu_body);
    flush(&mut patch);

    let rows: Vec<ControlId> = entries(&patch)
        .iter()
        .filter(|entry| !entry.flags.contains(HitFlags::BLOCKER))
        .map(|entry| entry.id)
        .collect();
    assert!(rows.len() >= 2, "two rows: {rows:?}");
    let (alpha, beta) = (rows[0], rows[1]);

    // Rest on the first row: a delay opens, and elapsing it opens the nested list.
    overlays.service(&[hover(Some(alpha))], &[], &hits(&patch), &mut focus);
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
    assert_eq!(overlays.depth(), 2, "the submenu did not open");
    flush(&mut patch);

    // Moving to a sibling row of the parent closes it. The sibling is in the parent menu,
    // which is what makes it a leave rather than a move *into* the submenu.
    overlays.service(&[hover(Some(beta))], &[], &hits(&patch), &mut focus);
    assert_eq!(
        overlays.depth(),
        1,
        "a hover-opened submenu outlived its row"
    );
    flush(&mut patch);

    overlays.close_all(&mut focus);
    flush(&mut patch);
}

#[test]
fn a_hover_open_closes_what_the_pointer_left_and_keeps_what_it_returned_to() {
    // Three levels, which is where "close the hover-opened ones" stops being the same
    // question as "close the first hover-opened one". Returning from a sub-submenu to a row
    // of the submenu must take the sub-submenu and leave the submenu: a rule stated against
    // the bottom of the stack finds the submenu, decides it is not above the pointer, and
    // closes nothing at all.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
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
    let _menu = overlays.open(Spec::popup(), &mut focus, top);
    flush(&mut patch);

    let rows = |patch: &SinkPatch| -> Vec<ControlId> {
        entries(patch)
            .iter()
            .filter(|entry| !entry.flags.contains(HitFlags::BLOCKER))
            .map(|entry| entry.id)
            .collect()
    };
    let mut dwell_open = |overlays: &mut Overlays, patch: &mut SinkPatch, target| {
        overlays.service(&[hover(Some(target))], &[], &hits(patch), &mut focus);
        let delay = pending_delay(patch);
        overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
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
    overlays.service(&[hover(Some(mid2))], &[], &hits(&patch), &mut focus);
    assert_eq!(
        overlays.depth(),
        2,
        "the level the pointer left stayed open, or the one it returned to was taken with it"
    );
    overlays.close_all(&mut focus);
    flush(&mut patch);
}

#[test]
fn escape_takes_the_description_before_the_menu_under_it() {
    // One keystroke closes one thing. The router raises `Escape` rather than a key wherever
    // a focus scope is open, so a menu with a description showing over it would otherwise
    // read the same press twice and lose both.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let menu_body = || {
        flyout().stack((
            button("Alpha").name("Alpha").tip("describe alpha"),
            button("Beta").name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus, menu_body);
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
        &hits(&patch),
        &mut focus,
    );
    let delay = pending_delay(&mut patch);
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
    flush(&mut patch);
    assert_eq!(overlays.depth(), 2, "the menu and its description");

    let escape = [Report::Escape { scope: None }];
    overlays.service(&escape, &[], &hits(&patch), &mut focus);
    assert_eq!(overlays.depth(), 1, "the menu went with the description");
    flush(&mut patch);

    // And the second one reaches the menu, so nothing is stranded by the first.
    overlays.service(&escape, &[], &hits(&patch), &mut focus);
    assert!(overlays.is_empty(), "a second Esc did not reach the menu");
    flush(&mut patch);
}

#[test]
fn closing_a_menu_cancels_the_dwell_it_started() {
    // A delay outliving its menu is not idle: it holds a frame-clock `Tick` for its whole
    // duration and then opens a submenu against a row that has gone. Every close path
    // reaches `truncate`, which is why this is asserted there rather than at each of them.
    let mut patch = fixture();
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let leaf = || flyout().stack(button("Leaf").name("Leaf"));
    let menu_body = move || {
        flyout().stack((
            button("Alpha").flyout(leaf).name("Alpha"),
            button("Beta").name("Beta"),
        ))
    };
    let _menu = overlays.open(Spec::popup(), &mut focus, menu_body);
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
        &hits(&patch),
        &mut focus,
    );
    let delay = pending_delay(&mut patch);

    overlays.close_all(&mut focus);
    assert_eq!(
        delay_ops(&mut patch),
        (0, 1),
        "closing left the delay running"
    );

    // And the one that was in flight reports against nothing.
    overlays.scene(&[SceneEvent::DelayElapsed { delay }], &mut focus);
    assert!(
        overlays.is_empty(),
        "a closed menu's dwell opened a submenu"
    );
    flush(&mut patch);
}

#[test]
fn a_clicked_flyout_does_not_close_because_the_pointer_moved() {
    // The other half of the same rule, and the reason `by_dwell` is recorded rather than
    // inferred: these two overlays are the same kind, opened the same way, differing only in
    // what opened them — and the right behaviour is opposite in each.
    let mut patch = fixture();
    let (_invoker, anchor) = invoker(&mut patch);
    let mut focus = FocusRing::default();
    let mut overlays = Overlays::new();

    let _open = overlays.open(Spec::flyout(anchor), &mut focus, body);
    flush(&mut patch);

    overlays.service(
        &[Report::HoverChanged {
            from: None,
            to: None,
            at: windows_scene::Point { x: 1.0, y: 1.0 },
            qpc: 0,
        }],
        &[],
        &hits(&patch),
        &mut focus,
    );
    assert_eq!(
        overlays.depth(),
        1,
        "a flyout the user clicked open closed because the pointer left it"
    );
    overlays.close_all(&mut focus);
    flush(&mut patch);
}

#[test]
fn dropping_the_stack_releases_a_pending_delay() {
    // A pending delay outlives the stack in two ways that both matter: its id stays claimed
    // in the model, and its batch keeps the frame clock awake. Neither is released by the
    // overlays going away unless the drop says so.
    let mut patch = fixture();
    let mut focus = FocusRing::default();

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
            &hits(&patch),
            &mut focus,
        );
        assert_eq!(delay_ops(&mut patch), (1, 0), "a delay is pending");
    }
    assert_eq!(delay_ops(&mut patch), (0, 1), "the drop cancelled it");

    drop(mount);
    flush(&mut patch);
}
