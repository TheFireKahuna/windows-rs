//! Exit transitions only fire when the transitioned node is actually
//! DESTROYED. `Element::Empty` is filtered from the live child list, so a
//! conditional child (`show.then(..)` / `unwrap_or(Element::Empty)`) reconciled
//! POSITIONALLY is morphed into its next sibling on hide — the last child gets
//! destroyed instead and the exit never plays. A stable `.with_key(..)` gives
//! the differ real identity: the hidden key unmounts, `destroy` fires on the
//! right node, and the backend can ghost it.

use std::rc::Rc;
use std::time::Duration;
use test_reactor::{Op, RecordingBackend};
use windows_reactor::*;

fn card(show: bool, keyed: bool) -> Element {
    let card: Element = if show {
        let b = border(button("card")).transition(
            None,
            Some(AnimationConfig::fade_out(Duration::from_millis(150))),
        );
        if keyed { b.with_key("card").into() } else { b.into() }
    } else {
        Element::Empty
    };
    let other = border(button("other"));
    let other: Element = if keyed { other.with_key("other").into() } else { other.into() };
    vstack((text_block("title"), button("toggle"), card, other)).into()
}

fn exit_registered_id(ops: &[Op]) -> Option<ControlId> {
    ops.iter().find_map(|op| match op {
        Op::SetExitTransition { id, config: Some(_) } => Some(*id),
        _ => None,
    })
}

fn destroyed_ids(ops: &[Op]) -> Vec<ControlId> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Destroy { id } => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn keyed_conditional_child_is_destroyed_and_can_ghost() {
    let mut r = Reconciler::new(RecordingBackend::new());
    let v1 = card(true, true);
    let id = r.reconcile(None, &v1, None, Rc::new(|| {})).unwrap();
    let exit_id = exit_registered_id(&r.backend.ops).expect("exit registered at mount");
    r.backend.clear_ops();

    let v2 = card(false, true);
    let _ = r.reconcile(Some(&v1), &v2, Some(id), Rc::new(|| {}));
    assert!(
        destroyed_ids(&r.backend.ops).contains(&exit_id),
        "keyed hide must destroy the exit-transitioned node"
    );
}

#[test]
fn unkeyed_conditional_child_morphs_instead_of_destroying() {
    // Documents the positional-differ behavior the example warns about: the
    // exit-transitioned node is NOT the one destroyed. If this ever starts
    // failing, positional semantics changed — revisit the `.with_key` guidance
    // in dcomp_animations.rs and the exit-ghost docs.
    let mut r = Reconciler::new(RecordingBackend::new());
    let v1 = card(true, false);
    let id = r.reconcile(None, &v1, None, Rc::new(|| {})).unwrap();
    let exit_id = exit_registered_id(&r.backend.ops).expect("exit registered at mount");
    r.backend.clear_ops();

    let v2 = card(false, false);
    let _ = r.reconcile(Some(&v1), &v2, Some(id), Rc::new(|| {}));
    assert!(
        !destroyed_ids(&r.backend.ops).contains(&exit_id),
        "positional hide morphs the node; exit does not fire (known semantics)"
    );
}
