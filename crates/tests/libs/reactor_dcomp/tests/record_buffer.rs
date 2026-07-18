//! `backend/dcomp/record.rs` — the command-buffer seam between the reconciler
//! and the composition tree, and the intent seam running the other way.
//!
//! Everything here is headless: the recorder is plain data plus a spy backend,
//! no HWND, no compositor, no window. The reconciler mints control ids in the
//! shipping pipeline, so these tests mint their own the same way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_reactor::dcomp_test_api::Recorder;
use windows_reactor::{
    Callback, ControlId, ControlKind, Event, EventHandler, PointerHandlers, Prop, PropValue,
    Tooltip,
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

/// `set_tooltip(_, None)` must round-trip as `None`, not as a dropped command:
/// the side-table key is optional and the clear path has no payload.
#[test]
fn side_table_round_trips_tooltip_set_then_clear() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);

    let tip = Tooltip::text("hello");
    rec.backend().set_tooltip(id(1), Some(&tip));
    rec.backend().set_tooltip(id(1), None);
    rec.flush();

    assert_eq!(
        rec.replayed_tooltips(),
        vec![Some(tip), None],
        "tooltip set/clear did not survive the buffer"
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
