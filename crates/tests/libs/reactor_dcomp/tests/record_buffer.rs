//! `backend/dcomp/record.rs` — the command-buffer seam between the reconciler
//! and the composition tree.
//!
//! Everything here is headless: the recorder is plain data plus a spy backend,
//! no HWND, no compositor, no window.

use std::cell::Cell;
use std::rc::Rc;

use windows_reactor::dcomp_test_api::Recorder;
use windows_reactor::{
    Callback, ControlId, ControlKind, Event, EventHandler, Prop, PropValue, Tooltip,
};

/// Control ids are minted monotonically and are NEVER reused.
///
/// The recorder owns the sole id counter for the process, and the arena keys
/// live nodes by it. A recycled id aliases a destroyed node, so the
/// reconciler's `children_mirror` / graft check (`new_id != old_id`) silently
/// stops firing: the freshly mounted subtree is never grafted and the destroyed
/// subtree's visuals stay on screen. This is the bug fixed in b90ef0677 — the
/// regression guard for the minting half of that contract.
#[test]
fn ids_are_monotonic_and_never_reused() {
    let mut rec = Recorder::new();

    let a = rec.backend().create(ControlKind::Border);
    let b = rec.backend().create(ControlKind::TextBlock);
    let c = rec.backend().create(ControlKind::Button);
    assert_eq!(
        (a.get(), b.get(), c.get()),
        (1, 2, 3),
        "ids must be sequential from 1"
    );

    // Destroy the middle one and let it actually reach the backend.
    rec.backend().destroy(b);
    rec.flush();

    // The next mint must skip the hole, not fill it.
    let d = rec.backend().create(ControlKind::Border);
    assert_eq!(d.get(), 4, "id {} reuses the destroyed slot", d.get());

    // And churn does not walk the counter backwards.
    let mut last = d.get();
    for _ in 0..64 {
        let id = rec.backend().create(ControlKind::Border);
        assert!(id.get() > last, "{} did not advance past {last}", id.get());
        last = id.get();
        rec.backend().destroy(id);
        rec.flush();
    }
}

/// Every backend call defers into the buffer, `create` included — so the
/// recorded stream is a complete encoding of the trait, with no call reaching
/// the backend ahead of replay.
///
/// `create` was once the sole eager exception, because `get_native_element`
/// took `&self` and so could not flush, and had to stay exact mid-reconcile.
/// The DComp backend no longer exposes native elements at all, so the exception
/// is gone. The id is still answered synchronously — it is minted from the
/// recorder's own counter, which is not a read-back of backend state.
#[test]
fn every_call_defers_including_create() {
    let mut rec = Recorder::new();

    let id = rec.backend().create(ControlKind::Border);
    assert_eq!(rec.pending(), 1, "create must buffer like every other call");
    assert!(
        rec.applied().is_empty(),
        "create must not reach the backend before flush: {:?}",
        rec.applied()
    );

    rec.backend()
        .set_prop(id, Prop::Opacity, &PropValue::F64(0.5));
    assert_eq!(rec.pending(), 2, "set_prop must buffer");
    assert!(
        rec.applied().is_empty(),
        "nothing may reach the backend before flush"
    );

    rec.flush();
    assert_eq!(rec.pending(), 0);
    let applied = rec.applied();
    assert_eq!(applied.len(), 2, "{applied:?}");
    assert_eq!(applied[0], format!("create {} Border", id.get()));
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
    let id = rec.backend().create(ControlKind::Border);

    rec.backend()
        .set_prop(id, Prop::Opacity, &PropValue::F64(0.25));
    rec.backend()
        .set_prop(id, Prop::Opacity, &PropValue::F64(0.50));
    rec.backend()
        .set_prop(id, Prop::Opacity, &PropValue::F64(1.00));
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
    let parent = rec.backend().create(ControlKind::StackPanel);
    let child = rec.backend().create(ControlKind::TextBlock);

    rec.backend().append_child(parent, child);
    rec.backend().destroy(child);
    rec.backend().remove_child(parent, 0);
    rec.flush();

    let applied = rec.applied();
    let tail: Vec<&str> = applied[2..].iter().map(String::as_str).collect();
    assert_eq!(
        tail,
        vec![
            format!("append {} {}", parent.get(), child.get()).as_str(),
            format!("destroy {}", child.get()).as_str(),
            format!("remove {} @0", parent.get()).as_str(),
        ],
        "replay reordered or repaired the buffer"
    );
}

/// Thread-affine payloads are parked in the side table and replayed verbatim.
///
/// Proven by *behaviour*, not by shape: the callback the test attached is the
/// callback the backend receives, so it still closes over the same cell.
#[test]
fn side_table_round_trips_an_event_handler() {
    let mut rec = Recorder::new();
    let id = rec.backend().create(ControlKind::Button);

    let fired = Rc::new(Cell::new(0u32));
    let f = Rc::clone(&fired);
    let handler = EventHandler::Unit(Callback::new(move |()| f.set(f.get() + 1)));

    rec.backend().attach_event(id, Event::Click, handler);
    assert_eq!(
        rec.pending(),
        2,
        "attach_event must buffer (behind the create)"
    );
    assert_eq!(fired.get(), 0);

    rec.flush();
    rec.invoke_replayed_event(0);
    assert_eq!(
        fired.get(),
        1,
        "the replayed handler is not the one that was parked"
    );
}

/// `set_tooltip(_, None)` must round-trip as `None`, not as a dropped command:
/// the side-table key is optional and the clear path has no payload.
#[test]
fn side_table_round_trips_tooltip_set_then_clear() {
    let mut rec = Recorder::new();
    let id = rec.backend().create(ControlKind::Button);

    let tip = Tooltip::text("hello");
    rec.backend().set_tooltip(id, Some(&tip));
    rec.backend().set_tooltip(id, None);
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
    let id = rec.backend().create(ControlKind::Border);
    rec.flush();
    let before = rec.applied();
    rec.flush();
    rec.flush();
    assert_eq!(
        rec.applied(),
        before,
        "repeated flush replayed the buffer twice"
    );
    let _ = id;
}

/// Nothing thread-affine has leaked into a `Cmd` variant. The lib asserts this
/// at compile time; this restates it where the test build can fail on it.
#[test]
fn command_buffer_payloads_are_send() {
    windows_reactor::dcomp_test_api::assert_cmd_buffer_is_send();
}

/// `ControlId` is a `NonZeroU32`, so id 0 can never be minted — the recorder's
/// counter is pre-incremented for exactly that reason.
#[test]
fn control_id_is_never_zero() {
    let mut rec = Recorder::new();
    for _ in 0..8 {
        let id = rec.backend().create(ControlKind::Border);
        assert_ne!(id.get(), 0);
    }
    assert!(std::panic::catch_unwind(|| ControlId::new(0)).is_err());
}
