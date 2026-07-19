//! `backend/dcomp/animate.rs` — the reduced-motion gate.
//!
//! The system animation preference (Accessibility → Visual effects → Animation
//! effects) has one obvious wrong implementation: skip the animation. It is
//! wrong because most of these animations do not decorate a state the element
//! is already in — they *establish* it. An enter transition runs opacity 0 → 1,
//! so a gate that returns early leaves the element at 0 and it never appears at
//! all. Reduced motion would make the app blank rather than still.
//!
//! So the contract these tests hold to is: **reduced motion changes the path,
//! never the destination.**
//!
//! Each case first writes a deliberately *wrong* value to the property with
//! [`ArenaHarness::set_visual_state`], then animates away from it. That step is
//! load-bearing and was missing from the first draft of this file: a visual is
//! born at opacity 1.0 and scale 1.0, which is precisely where a fade-in or a
//! pop-in is trying to land — so a test that animates a newborn visual asserts
//! the birth value and passes whether the gate settles or skips. Mutating
//! `settle` to a bare `return` proved it: only the fade-*out* case failed.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::{AnimationConfig, ControlKind as K, ImplicitTransitions, ScalarTransition};

/// The motion preference is a process-global, so these tests cannot run
/// concurrently with each other — one would restore it out from under another.
static SERIAL: Mutex<()> = Mutex::new(());

fn harness() -> (ArenaHarness, MutexGuard<'static, ()>) {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let h = ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it",
    );
    (h, guard)
}

/// A fade-in: from fully transparent to fully opaque.
fn fade_in() -> AnimationConfig {
    AnimationConfig {
        opacity: Some(1.0),
        from_opacity: Some(0.0),
        duration: Duration::from_millis(300),
        ..Default::default()
    }
}

/// The whole point. A fade-in under reduced motion must END opaque; the
/// `from_opacity: 0.0` is what a skipping gate would leave behind.
#[test]
fn a_fade_in_still_ends_opaque_under_reduced_motion() {
    let (mut h, _s) = harness();
    h.set_reduced_motion(true);

    let id = h.insert(K::Button).unwrap();
    h.set_visual_state(id, 0.0, 1.0); // start transparent, as the enter would
    h.run_animation(id, &fade_in());

    assert_eq!(
        h.opacity(id),
        Some(1.0),
        "reduced motion skipped the effect instead of settling it — the element \
         is left at its `from` opacity and never becomes visible"
    );
    h.set_reduced_motion(false);
}

/// Same contract on the other animated property: a pop-in that scales 0.8 → 1.0
/// must end at 1.0, not stay shrunken.
#[test]
fn a_scale_in_still_ends_at_full_size_under_reduced_motion() {
    let (mut h, _s) = harness();
    h.set_reduced_motion(true);

    let id = h.insert(K::Button).unwrap();
    h.set_visual_state(id, 1.0, 0.8); // start shrunken, as the pop-in would
    h.run_animation(
        id,
        &AnimationConfig {
            scale: Some(1.0),
            from_scale: Some(0.8),
            ..Default::default()
        },
    );

    assert_eq!(h.scale(id), Some(1.0), "reduced motion left the node shrunken");
    h.set_reduced_motion(false);
}

/// A fade-OUT settles too. This is the direction where skipping looks correct
/// on screen (the element is about to be removed) — but the property is the
/// same one an exit ghost reads, so it must still land where it was told.
#[test]
fn a_fade_out_still_ends_transparent_under_reduced_motion() {
    let (mut h, _s) = harness();
    h.set_reduced_motion(true);

    let id = h.insert(K::Button).unwrap();
    h.run_animation(
        id,
        &AnimationConfig {
            opacity: Some(0.0),
            from_opacity: Some(1.0),
            ..Default::default()
        },
    );

    assert_eq!(h.opacity(id), Some(0.0));
    h.set_reduced_motion(false);
}

/// Reduced motion must attach no implicit collection: with none, the ordinary
/// prop and layout writers take effect immediately, which is exactly the
/// wanted behaviour and needs no separate code path.
#[test]
fn reduced_motion_attaches_no_implicit_collection() {
    let (mut h, _s) = harness();
    let transitions = ImplicitTransitions {
        opacity: Some(ScalarTransition::new(Duration::from_millis(200))),
        ..Default::default()
    };

    h.set_reduced_motion(true);
    let id = h.insert(K::Button).unwrap();
    h.rebuild_implicit(id, transitions);
    assert_eq!(
        h.has_implicit(id),
        Some(false),
        "an implicit collection under reduced motion keeps every prop change gliding"
    );
    h.set_reduced_motion(false);
}

/// ...and the flip is live in BOTH directions. A node built while motion was
/// reduced must gain its collection back when the user turns animation on,
/// which is the half a one-way gate would miss: `refresh_motion` rebuilds, and
/// `build_implicit` returning `Some` again is what makes that rebuild mean
/// something.
#[test]
fn turning_motion_back_on_restores_the_implicit_collection() {
    let (mut h, _s) = harness();
    let transitions = ImplicitTransitions {
        opacity: Some(ScalarTransition::new(Duration::from_millis(200))),
        ..Default::default()
    };

    h.set_reduced_motion(true);
    let id = h.insert(K::Button).unwrap();
    h.rebuild_implicit(id, transitions);
    assert_eq!(h.has_implicit(id), Some(false));

    // The user flips the preference; `refresh_motion` re-runs this per node.
    h.set_reduced_motion(false);
    h.rebuild_implicit(id, transitions);
    assert_eq!(
        h.has_implicit(id),
        Some(true),
        "the collection never came back — motion stays dead until the node is rebuilt"
    );
}
