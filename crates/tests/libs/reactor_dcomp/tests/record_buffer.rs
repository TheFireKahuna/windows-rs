//! `backend/dcomp/record.rs` — the command-buffer seam between the reconciler
//! and the composition tree, and the intent seam running the other way.
//!
//! Everything here is headless: the recorder is plain data plus a spy backend,
//! no HWND, no compositor, no window. The reconciler mints control ids in the
//! shipping pipeline, so these tests mint their own the same way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use windows_reactor::dcomp_test_api::{GestureHarness, Recorder};
use windows_reactor::{ActionSlot, GestureEvent, GestureInterest, GestureOutcome};
use windows_reactor::{
    Callback, ControlId, ControlKind, Event, EventHandler, KeyboardAccelerator, PointerEventInfo,
    PointerHandlers, Prop, PropValue, Tooltip, VirtualKey, VirtualKeyModifiers,
};

fn id(raw: u32) -> ControlId {
    ControlId::new(raw)
}

/// Every backend call defers into the buffer, `create` included — so the
/// recorded stream is a complete encoding of the trait, with no call reaching
/// the backend ahead of replay. (The reconciler mints the id; nothing about a
/// `create` is a read-back of backend state.)
#[test]
fn every_call_defers_including_create() {
    let mut rec = Recorder::new();

    rec.backend().create(id(1), ControlKind::Border);
    assert_eq!(rec.pending(), 1, "create must buffer like every other call");
    assert!(
        rec.applied().is_empty(),
        "create must not reach the backend before flush: {:?}",
        rec.applied()
    );

    rec.backend()
        .set_prop(id(1), Prop::Opacity, &PropValue::F64(0.5));
    assert_eq!(rec.pending(), 2, "set_prop must buffer");
    assert!(
        rec.applied().is_empty(),
        "nothing may reach the backend before flush"
    );

    rec.flush();
    assert_eq!(rec.pending(), 0);
    let applied = rec.applied();
    assert_eq!(applied.len(), 2, "{applied:?}");
    assert_eq!(applied[0], "create 1 Border");
    assert!(applied[1].starts_with("set_prop"), "{applied:?}");
}

/// Replay is literal: same order, no coalescing, no de-duplication.
///
/// Coalescing two writes to the same prop looks harmless and is not — the
/// reconciler is the authority on tree shape and issues intermediate states
/// deliberately.
#[test]
fn replay_preserves_order_and_does_not_coalesce() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Border);

    rec.backend()
        .set_prop(id(1), Prop::Opacity, &PropValue::F64(0.25));
    rec.backend()
        .set_prop(id(1), Prop::Opacity, &PropValue::F64(0.50));
    rec.backend()
        .set_prop(id(1), Prop::Opacity, &PropValue::F64(1.00));
    assert_eq!(
        rec.pending(),
        4,
        "identical-prop writes must not collapse in the buffer (plus the create)"
    );

    rec.flush();
    let applied = rec.applied();
    assert_eq!(
        applied.len(),
        4,
        "the create and all three writes must replay: {applied:?}"
    );
    assert!(applied[1].contains("0.25"), "{applied:?}");
    assert!(applied[2].contains("0.5"), "{applied:?}");
    assert!(applied[3].contains("1.0"), "{applied:?}");
}

/// The buffer legitimately carries transient states the reconciler produces —
/// a child destroyed *before* it is unparented. Replay must not "repair" them.
#[test]
fn replay_keeps_transient_destroy_before_unparent() {
    let mut rec = Recorder::new();
    let (parent, child) = (id(1), id(2));
    rec.backend().create(parent, ControlKind::StackPanel);
    rec.backend().create(child, ControlKind::TextBlock);

    rec.backend().append_child(parent, child);
    rec.backend().destroy(child);
    rec.backend().remove_child(parent, 0);
    rec.flush();

    let applied = rec.applied();
    let tail: Vec<&str> = applied[2..].iter().map(String::as_str).collect();
    assert_eq!(
        tail,
        vec!["append 1 2", "destroy 2", "remove 1 @0"],
        "replay reordered or repaired the buffer"
    );
}

/// An event handler never crosses the seam: `attach_event` records a pure
/// `{id, event}` declaration and the closure stays in the recorder's app-side
/// map, where the intent drain finds and invokes it.
#[test]
fn event_handler_stays_app_side_and_fires_from_intents() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let fired = Rc::new(Cell::new(0u32));
    let f = Rc::clone(&fired);
    let handler = EventHandler::Unit(Callback::new(move |()| f.set(f.get() + 1)));

    rec.backend().attach_event(id(1), Event::Click, handler);
    assert_eq!(
        rec.pending(),
        2,
        "attach_event must buffer (behind the create)"
    );
    rec.flush();
    let applied = rec.applied();
    assert_eq!(
        applied[1], "declare_event 1 Click",
        "replay must declare, never hand the closure to the backend: {applied:?}"
    );

    // The backend queues a Click intent; the drain resolves it against the
    // app-side map and runs the handler.
    rec.queue_unit_event(id(1), Event::Click);
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(fired.get(), 1, "the mapped handler did not run");

    // An intent for an event nobody subscribed to resolves to nothing.
    rec.queue_unit_event(id(1), Event::Toggled);
    assert_eq!(rec.drain_and_run(), 0);

    // Detach removes the mapping: the same intent now resolves to nothing.
    rec.backend().detach_event(id(1), Event::Click);
    rec.queue_unit_event(id(1), Event::Click);
    assert_eq!(rec.drain_and_run(), 0, "detached handler still ran");
    assert_eq!(fired.get(), 1);
}

/// The intent queue is FIFO: a node with both a `Click` handler and
/// `on_tapped` observes them in exactly the order they were queued — the
/// Click-then-tapped contract a single activation produces.
#[test]
fn intent_order_click_then_tapped() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Border);

    let order = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let o1 = Rc::clone(&order);
    rec.backend().attach_event(
        id(1),
        Event::Click,
        EventHandler::Unit(Callback::new(move |()| o1.borrow_mut().push("click"))),
    );
    let o2 = Rc::clone(&order);
    let handlers = PointerHandlers {
        on_tapped: Some(Callback::new(move |()| o2.borrow_mut().push("tapped"))),
        ..PointerHandlers::default()
    };
    rec.backend().set_pointer_handlers(id(1), Some(&handlers));
    rec.flush();
    assert!(
        rec.applied()
            .iter()
            .any(|l| l.starts_with("set_pointer_interest 1") && l.contains("tapped: true")),
        "pointer presence bits must replay: {:?}",
        rec.applied()
    );

    rec.queue_unit_event(id(1), Event::Click);
    rec.queue_tapped(id(1));
    assert_eq!(rec.drain_and_run(), 2);
    assert_eq!(
        *order.borrow(),
        vec!["click", "tapped"],
        "queue order must be invocation order"
    );
}

/// The §7.2 revision protocol for control values, recorder half: a numeric
/// `Prop::Value` write is stamped with the latest `ValueChanged` revision the
/// drain has delivered for that control — `based_on = 0` before any input,
/// then the delivered revision after.
#[test]
fn value_writes_are_stamped_with_delivered_revision() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Slider);

    let seen = Rc::new(RefCell::new(Vec::<f64>::new()));
    let s = Rc::clone(&seen);
    rec.backend().attach_event(
        id(1),
        Event::ValueChanged,
        EventHandler::F64(Callback::new(move |v: f64| s.borrow_mut().push(v))),
    );

    // Before any input, an app write is stamped against revision 0.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::F64(1.0));

    // Input delivered revision 7; the app's next write is based on it.
    rec.queue_value_changed(id(1), 4.5, 7);
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(*seen.borrow(), vec![4.5], "ValueChanged handler payload");
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::F64(4.5));

    rec.flush();
    let applied = rec.applied();
    let stamps: Vec<&str> = applied
        .iter()
        .filter(|l| l.starts_with("set_value"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        stamps,
        vec!["set_value 1 1 based_on=0", "set_value 1 4.5 based_on=7"],
        "value writes must carry the delivered revision: {applied:?}"
    );
}

/// A tooltip is a declaration, not a payload: the full `Tooltip` (a rich one
/// holds an `Element` tree) stays in the recorder's app-side map, the buffer
/// carries only the `Send` declaration, and replay routes **nothing** to the
/// backend's `set_tooltip` — this backend has no tooltip presenter yet, and
/// when it grows one it will consume the declaration, never the payload.
#[test]
fn tooltip_payload_never_reaches_the_backend() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let tip = Tooltip::text("hello");
    rec.backend().set_tooltip(id(1), Some(&tip));
    rec.backend().set_tooltip(id(1), None);
    assert_eq!(
        rec.pending(),
        3,
        "set and clear must each buffer a declaration"
    );
    rec.flush();

    assert_eq!(
        rec.replayed_tooltips(),
        Vec::<Option<Tooltip>>::new(),
        "the tooltip payload crossed the seam — it must stay app-side"
    );
}

/// A flush with nothing buffered must not manufacture work.
#[test]
fn empty_flush_is_a_no_op() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Border);
    rec.flush();
    let before = rec.applied();
    rec.flush();
    rec.flush();
    assert_eq!(
        rec.applied(),
        before,
        "repeated flush replayed the buffer twice"
    );
}

/// A gesture runs **inline, on the routing thread** — the property the whole
/// front-hosted design exists to guarantee. Every transition reaches it during
/// delivery, before any drain, so whatever it publishes is committed before the
/// app has heard that input happened at all.
#[test]
fn a_gesture_runs_inline_during_delivery_not_at_drain() {
    let mut rec = Recorder::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let l = Arc::clone(&log);
    let _g = GestureHarness::register(
        id(1),
        GestureInterest::drag(),
        move |ev: GestureEvent| {
            l.lock().unwrap().push(match ev {
                GestureEvent::Down(i) => format!("down@{}", i.x),
                GestureEvent::Move(i) => format!("move@{}", i.x),
                GestureEvent::Up(i) => format!("up@{}", i.x),
                GestureEvent::Wheel(i) => format!("wheel@{}", i.x),
                GestureEvent::Exit => "exit".to_string(),
            });
            GestureOutcome::Handled
        },
        || panic!("a Handled gesture must never notify the app"),
    );

    let at = |x: f64| PointerEventInfo { x, ..Default::default() };
    rec.deliver_gesture(id(1), GestureEvent::Down(at(1.0)));
    rec.deliver_gesture(id(1), GestureEvent::Move(at(2.0)));
    rec.deliver_gesture(id(1), GestureEvent::Up(at(3.0)));

    assert_eq!(
        &*log.lock().unwrap(),
        &["down@1", "move@2", "up@3"],
        "the gesture must see every transition, in order, at delivery time"
    );
    assert_eq!(
        rec.drain_and_run(),
        0,
        "a gesture that handled its own visual must stage no app work"
    );
}

/// The coalescing contract: a burst of moves between two app turns wakes the
/// app **once** and delivers only the newest action.
///
/// This is what makes the notification path cheapen under load instead of
/// dearer — the sink design it replaces posted one intent per move.
#[test]
fn a_burst_of_moves_wakes_the_app_once_with_the_newest_action() {
    let mut rec = Recorder::new();
    let slot = Arc::new(ActionSlot::<f64>::new());
    let drained = Rc::new(RefCell::new(Vec::<f64>::new()));

    let post = Arc::clone(&slot);
    let d = Rc::clone(&drained);
    let take = Arc::clone(&slot);
    let _g = GestureHarness::register(
        id(1),
        GestureInterest::drag(),
        move |ev: GestureEvent| match ev {
            GestureEvent::Move(i) => post.post(i.x),
            _ => GestureOutcome::Handled,
        },
        move || {
            if let Some(v) = take.take() {
                d.borrow_mut().push(v);
            }
        },
    );

    let at = |x: f64| PointerEventInfo { x, ..Default::default() };
    let notified: Vec<bool> = (1..=5)
        .map(|x| rec.deliver_gesture(id(1), GestureEvent::Move(at(f64::from(x)))))
        .collect();

    assert_eq!(
        notified,
        vec![true, false, false, false, false],
        "only the first post of a burst may stage a wake"
    );
    assert_eq!(rec.drain_and_run(), 1, "five moves must resolve to one app job");
    assert_eq!(
        &*drained.borrow(),
        &[5.0],
        "the app must see only the newest action, not the whole burst"
    );

    // Drained, so the next burst re-arms.
    assert!(rec.deliver_gesture(id(1), GestureEvent::Move(at(6.0))));
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(&*drained.borrow(), &[5.0, 6.0]);
}

/// Hover-exit reaches the gesture like any other transition, and a forgotten
/// gesture receives nothing — neither the handler nor its app-side drain.
#[test]
fn exit_reaches_the_gesture_and_forgetting_is_honoured() {
    let mut rec = Recorder::new();
    let exits = Arc::new(Mutex::new(0u32));

    {
        let e = Arc::clone(&exits);
        let _g = GestureHarness::register(
            id(1),
            GestureInterest::hover(),
            move |ev: GestureEvent| {
                if matches!(ev, GestureEvent::Exit) {
                    *e.lock().unwrap() += 1;
                }
                GestureOutcome::Handled
            },
            || {},
        );
        rec.deliver_gesture(id(1), GestureEvent::Exit);
        assert_eq!(*exits.lock().unwrap(), 1, "the gesture did not see its exit");
        // `_g` drops here, forgetting the gesture.
    }

    assert!(
        !rec.deliver_gesture(id(1), GestureEvent::Exit),
        "a forgotten gesture must stage nothing"
    );
    assert_eq!(*exits.lock().unwrap(), 1, "a forgotten gesture must not run");
    assert_eq!(rec.drain_and_run(), 0);
}

/// A hover-only gesture stays **click-transparent**: it declares no `down`, so
/// the router lets a press fall through to whatever lies beneath — a button
/// layered over a plot keeps working.
#[test]
fn a_hover_gesture_declares_no_press_interest() {
    let _rec = Recorder::new();
    let g = GestureHarness::register(id(1), GestureInterest::hover(), |_| GestureOutcome::Handled, || {});

    let interest = g.interest().expect("a declared gesture must be routable");
    assert!(!interest.down, "a hover gesture must not claim presses");
    assert!(!interest.up && !interest.wheel);
    assert!(interest.moved && interest.exited);
}

/// §7.3 accelerators, declaration half: `set_keyboard_accelerators` records a
/// pure `(key, mods)` list into the buffer, and replay hands the front only
/// that list (`set_keybindings`) — never the `on_invoked` closures, which stay
/// in the recorder's app-side `accels` map. An empty list clears the entry.
#[test]
fn keybindings_declaration_reaches_the_front_via_replay() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let accels = vec![
        KeyboardAccelerator::new(VirtualKey::S, VirtualKeyModifiers::Control, || {}),
        KeyboardAccelerator::new(VirtualKey::F5, VirtualKeyModifiers::None, || {}),
    ];
    rec.backend().set_keyboard_accelerators(id(1), &accels);
    rec.flush();

    let applied = rec.applied();
    assert!(
        applied.iter().any(|l| l == "set_keybindings 1 2"),
        "the chord list must replay as a front declaration: {applied:?}"
    );
    assert!(
        !applied.iter().any(|l| l.contains("on_invoked") || l.contains("Callback")),
        "no accelerator callback may cross the seam: {applied:?}"
    );

    // Clearing the accelerators replays an empty declaration.
    rec.backend().set_keyboard_accelerators(id(1), &[]);
    rec.flush();
    assert!(
        rec.applied().iter().any(|l| l == "set_keybindings 1 0"),
        "clearing must replay an empty list: {:?}",
        rec.applied()
    );
}

/// §7.3 accelerators, fire half: an `Intent::Accelerator { id, index }` resolves
/// against the recorder's app-side `accels` map **by index**, so a node with
/// several chords invokes exactly the one the front matched.
#[test]
fn accelerator_intent_resolves_to_callback_by_index() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let fired = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let f0 = Rc::clone(&fired);
    let f1 = Rc::clone(&fired);
    let accels = vec![
        KeyboardAccelerator::new(VirtualKey::S, VirtualKeyModifiers::Control, move || {
            f0.borrow_mut().push("save");
        }),
        KeyboardAccelerator::new(VirtualKey::O, VirtualKeyModifiers::Control, move || {
            f1.borrow_mut().push("open");
        }),
    ];
    rec.backend().set_keyboard_accelerators(id(1), &accels);

    // The front matched the second chord (index 1) — only "open" must fire.
    rec.queue_accelerator(id(1), 1);
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(*fired.borrow(), vec!["open"], "wrong accelerator by index");

    // An index past the declared list resolves to nothing (never panics).
    rec.queue_accelerator(id(1), 9);
    assert_eq!(rec.drain_and_run(), 0);
    assert_eq!(*fired.borrow(), vec!["open"]);
}

/// A destroyed node's accelerators die with it: the recorder drops the
/// `accels` entry on `destroy`, so a later `Intent::Accelerator` for that id
/// resolves to nothing (ids are never reused, so it can never re-address a
/// live node).
#[test]
fn destroyed_node_accelerators_die() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let fired = Rc::new(Cell::new(0u32));
    let f = Rc::clone(&fired);
    rec.backend().set_keyboard_accelerators(
        id(1),
        &[KeyboardAccelerator::new(
            VirtualKey::S,
            VirtualKeyModifiers::Control,
            move || f.set(f.get() + 1),
        )],
    );

    // Live: the accelerator fires.
    rec.queue_accelerator(id(1), 0);
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(fired.get(), 1);

    // Destroyed: the same intent resolves to nothing.
    rec.backend().destroy(id(1));
    rec.queue_accelerator(id(1), 0);
    assert_eq!(rec.drain_and_run(), 0, "a destroyed node's accelerator still fired");
    assert_eq!(fired.get(), 1);
}

/// The §7.2 revision protocol for editor text, recorder half: a `TextBox`'s
/// string `Prop::Value` write becomes a stamped `SetText`, a write matching
/// the recorder's view of the front buffer is not re-recorded (the reconciler
/// re-emits the binding every render — the dedupe here is what keeps the
/// steady state silent), and a write that *disagrees* with a delivered edit —
/// the app declining it, "revert to the same prop value" — crosses as a
/// corrective `SetText` stamped with the delivered revision.
#[test]
fn text_writes_are_stamped_and_view_deduped() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::TextBox);

    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let s = Rc::clone(&seen);
    rec.backend().attach_event(
        id(1),
        Event::TextChanged,
        EventHandler::Str(Callback::new(move |t: String| s.borrow_mut().push(t))),
    );

    // Mount seed: stamped against revision 0.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::Str("seed".into()));
    // The reconciler re-emits the same binding next render: deduped.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::Str("seed".into()));

    // The user types; the app is consulted at revision 3.
    rec.queue_editor_text(id(1), Event::TextChanged, "typed", 3);
    assert_eq!(rec.drain_and_run(), 1);
    assert_eq!(*seen.borrow(), vec!["typed".to_string()]);

    // The app's verbatim echo of what it was shown: deduped, no wire traffic.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::Str("typed".into()));
    // The app declines the edit and re-declares other text: a corrective
    // write, stamped with the revision it was consulted at.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::Str("seed".into()));

    rec.flush();
    let applied = rec.applied();
    let texts: Vec<&str> = applied
        .iter()
        .filter(|l| l.starts_with("set_text"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        texts,
        vec![
            "set_text 1 \"seed\" based_on=0",
            "set_text 1 \"seed\" based_on=3",
        ],
        "duplicate writes must dedupe and corrective writes must cross stamped: {applied:?}"
    );
}

/// An editor nobody listens to keeps revision 0 at the recorder: the drain
/// advances `delivered_text_rev` only when a handler exists, so a later
/// programmatic write cannot arrive stamped fresh against user text the app
/// was never told about — the front drops it instead of retracting the edit.
#[test]
fn unconsulted_editor_revision_never_advances() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::TextBox);

    // The user typed at revisions 1..=5, but no handler is attached.
    rec.queue_editor_text(id(1), Event::TextChanged, "user text", 5);
    assert_eq!(rec.drain_and_run(), 0, "no handler must run");

    // An unrelated re-render re-emits the app's stale declaration. It must be
    // stamped 0 (never consulted), which the front's revision gate drops.
    rec.backend()
        .set_prop(id(1), Prop::Value, &PropValue::Str("stale".into()));
    rec.flush();
    let applied = rec.applied();
    assert!(
        applied.iter().any(|l| l == "set_text 1 \"stale\" based_on=0"),
        "an unconsulted editor's write must be stamped 0: {applied:?}"
    );
}

/// An `AutoSuggestBox` declares its text via `Prop::Text` (not `Prop::Value`);
/// the recorder's kind gate routes exactly that prop through `SetText`, while
/// `Prop::Text` on a label kind stays an ordinary prop write.
#[test]
fn text_prop_routes_by_kind() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::AutoSuggestBox);
    rec.backend().create(id(2), ControlKind::TextBlock);

    rec.backend()
        .set_prop(id(1), Prop::Text, &PropValue::Str("query".into()));
    rec.backend()
        .set_prop(id(2), Prop::Text, &PropValue::Str("label".into()));

    rec.flush();
    let applied = rec.applied();
    assert!(
        applied.iter().any(|l| l == "set_text 1 \"query\" based_on=0"),
        "AutoSuggestBox text must ride SetText: {applied:?}"
    );
    assert!(
        applied
            .iter()
            .any(|l| l.starts_with("set_prop") && l.contains("label")),
        "a TextBlock's text must stay an ordinary prop write: {applied:?}"
    );
}

/// Nothing thread-affine has leaked into a `Cmd` or `Intent` variant. The lib
/// asserts this at compile time; this restates it where the test build can
/// fail on it.
#[test]
fn command_buffer_payloads_are_send() {
    windows_reactor::dcomp_test_api::assert_cmd_buffer_is_send();
}

/// `ControlId` is a `NonZeroU32`, so id 0 can never be minted.
#[test]
fn control_id_is_never_zero() {
    assert!(std::panic::catch_unwind(|| ControlId::new(0)).is_err());
}
