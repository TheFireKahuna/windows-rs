//! `backend/dcomp/record.rs` — the flyout's split across the command-buffer
//! seam.
//!
//! A `FlyoutDef` holds a `Box<Element>` and an `on_closed` callback, neither of
//! which is `Send`, so it cannot ride the buffer whole. It splits: the plain
//! data the popup is built from crosses as a `FlyoutDecl`, and the element tree
//! and the callback stay in the recorder's app-side map keyed by owner id.
//!
//! These tests are the contract on that split. They are headless — the recorder
//! plus a spy backend, no HWND, no compositor.

use std::cell::Cell;
use std::rc::Rc;

use windows_reactor::dcomp_test_api::Recorder;
use windows_reactor::{
    Callback, ControlId, ControlKind, FlyoutDef, FlyoutPlacementMode, Prop, PropValue, text_block,
};

fn id(raw: u32) -> ControlId {
    ControlId::new(raw)
}

/// The declaration carries the popup's plain data — and nothing else.
#[test]
fn the_declaration_carries_the_plain_data() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);
    rec.backend().set_prop(
        id(1),
        Prop::FlyoutContent,
        &PropValue::FlyoutDef(
            FlyoutDef::text("Applies to every channel.")
                .placement(FlyoutPlacementMode::Right),
        ),
    );
    rec.flush();

    let decls = rec.replayed_flyouts();
    assert_eq!(decls.len(), 1, "{decls:?}");
    let d = decls[0].as_ref().expect("a flyout was set, not cleared");
    assert_eq!(d.text, "Applies to every channel.");
    assert_eq!(d.placement, FlyoutPlacementMode::Right.0);
    assert!(!d.rich, "a text flyout has no element tree");
    assert!(!d.notifies_closed, "no on_closed was registered");
    assert_eq!(d.open, None, "uncontrolled visibility");
}

/// A rich flyout declares its element tree's PRESENCE and never its content.
///
/// An `Element` can only become pixels by going back through the reconciler, so
/// shipping one across the buffer would be both impossible (`!Send`) and
/// useless. The bit is what lets the front know to ask for realization.
#[test]
fn a_rich_flyout_declares_presence_not_content() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);
    rec.backend().set_prop(
        id(1),
        Prop::FlyoutContent,
        &PropValue::FlyoutDef(FlyoutDef::rich(text_block("live"))),
    );
    rec.flush();

    let decls = rec.replayed_flyouts();
    let d = decls[0].as_ref().expect("a flyout was set");
    assert!(d.rich, "the presence bit must cross");
    assert!(
        d.text.is_empty(),
        "a rich flyout has no text, and must not invent one"
    );
}

/// `on_closed` never crosses the seam; a dismissal intent resolves against the
/// app-side map instead.
#[test]
fn on_closed_stays_app_side_and_fires_from_an_intent() {
    let mut rec = Recorder::new();
    let fired = Rc::new(Cell::new(0));
    let f = Rc::clone(&fired);

    rec.backend().create(id(1), ControlKind::Button);
    rec.backend().set_prop(
        id(1),
        Prop::FlyoutContent,
        &PropValue::FlyoutDef(
            FlyoutDef::text("hi").on_closed(Callback::new(move |()| f.set(f.get() + 1))),
        ),
    );
    rec.flush();

    let d = rec.replayed_flyouts()[0].clone().expect("a flyout was set");
    assert!(
        d.notifies_closed,
        "the front must know to queue a dismissal intent"
    );
    assert_eq!(fired.get(), 0, "declaring must not invoke");

    rec.queue_flyout_closed(id(1));
    assert_eq!(rec.drain_and_run(), 1, "the dismissal must resolve to the callback");
    assert_eq!(fired.get(), 1);
}

/// Clearing the flyout clears BOTH sides together.
///
/// If the declaration were dropped while the app-side entry survived, a later
/// dismissal intent for a reused id would fire a callback the app had already
/// removed.
#[test]
fn clearing_drops_the_declaration_and_the_callback_together() {
    let mut rec = Recorder::new();
    let fired = Rc::new(Cell::new(0));
    let f = Rc::clone(&fired);

    rec.backend().create(id(1), ControlKind::Button);
    rec.backend().set_prop(
        id(1),
        Prop::FlyoutContent,
        &PropValue::FlyoutDef(
            FlyoutDef::text("hi").on_closed(Callback::new(move |()| f.set(f.get() + 1))),
        ),
    );
    rec.backend()
        .set_prop(id(1), Prop::FlyoutContent, &PropValue::Unset);
    rec.flush();

    let decls = rec.replayed_flyouts();
    assert_eq!(decls.len(), 2, "set then clear are two declarations: {decls:?}");
    assert!(decls[1].is_none(), "the clear must cross as `None`");

    rec.queue_flyout_closed(id(1));
    assert_eq!(
        rec.drain_and_run(),
        0,
        "the callback was removed with the declaration"
    );
    assert_eq!(fired.get(), 0);
}

/// Destroying the owner drops its app-side flyout entry, so a dismissal that
/// races the teardown resolves to nothing rather than a stale callback.
#[test]
fn destroying_the_owner_drops_its_flyout() {
    let mut rec = Recorder::new();
    let fired = Rc::new(Cell::new(0));
    let f = Rc::clone(&fired);

    rec.backend().create(id(1), ControlKind::Button);
    rec.backend().set_prop(
        id(1),
        Prop::FlyoutContent,
        &PropValue::FlyoutDef(
            FlyoutDef::text("hi").on_closed(Callback::new(move |()| f.set(f.get() + 1))),
        ),
    );
    rec.backend().destroy(id(1));
    rec.flush();

    rec.queue_flyout_closed(id(1));
    assert_eq!(rec.drain_and_run(), 0);
    assert_eq!(fired.get(), 0);
}

/// The text shorthand and the full def land as the same declaration shape, so
/// the front has one thing to consume rather than two.
#[test]
fn the_text_shorthand_lands_as_a_full_declaration() {
    let mut rec = Recorder::new();
    rec.backend().create(id(1), ControlKind::Button);
    rec.backend()
        .set_prop(id(1), Prop::FlyoutContent, &PropValue::Str("more".into()));
    rec.flush();

    let d = rec.replayed_flyouts()[0].clone().expect("a flyout was set");
    assert_eq!(d.text, "more");
    assert!(!d.rich);
}
