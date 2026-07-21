//! `backend/dcomp/input.rs` — the §7.3 keyboard consumption policy and the
//! NumberBox Escape-revert.
//!
//! The two synchronous decisions §7.3 keeps front-side (the editor-vs-
//! accelerator conflict rule and the sys-key `return-0`-vs-`DefWindowProc`
//! choice) are pure functions; a WndProc is unreachable headless, so these
//! tests drive the shipping decision directly. The Escape-revert needs a real
//! editor buffer, so it runs on the (non-headless) `ArenaHarness` — see
//! `node_ctrl_lazy.rs` for why that needs a windowless compositor.

use windows_reactor::dcomp_test_api::{
    editor_claims_key, is_function_key, sys_key_falls_through, ArenaHarness,
};
use windows_reactor::ControlKind;

// The raw virtual-key codes the WndProc forwards (== `VirtualKey` integers).
const VK_A: u32 = 0x41;
const VK_C: u32 = 0x43;
const VK_S: u32 = 0x53;
const VK_V: u32 = 0x56;
const VK_X: u32 = 0x58;
const VK_BACK: u32 = 0x08;
const VK_LEFT: u32 = 0x25;
const VK_F1: u32 = 0x70;
const VK_F5: u32 = 0x74;
const VK_F24: u32 = 0x87;

/// The sys-key fallthrough decision: only an **unconsumed sys-key** reaches
/// `DefWindowProc`. A regular key never does (return 0 as before); a consumed
/// sys-key (an accelerator match, an editor claim) stays swallowed. This is the
/// exact branch that makes Alt+F4 / F10 / Alt+Space work again.
#[test]
fn sys_key_falls_through_only_when_unconsumed() {
    assert!(sys_key_falls_through(true, false), "unconsumed sys-key must reach DefWindowProc");
    assert!(!sys_key_falls_through(true, true), "a consumed sys-key stays swallowed");
    assert!(!sys_key_falls_through(false, false), "a regular key never falls through");
    assert!(!sys_key_falls_through(false, true), "a consumed regular key never falls through");
}

/// F1..F24 are the function keys — never editor-claimed, always available to an
/// accelerator or the system.
#[test]
fn is_function_key_covers_f1_to_f24() {
    assert!(is_function_key(VK_F1));
    assert!(is_function_key(VK_F5));
    assert!(is_function_key(VK_F24));
    assert!(!is_function_key(VK_F1 - 1), "0x6F is not a function key");
    assert!(!is_function_key(VK_F24 + 1), "0x88 is not a function key");
    assert!(!is_function_key(VK_A));
}

/// The fixed editor-vs-accelerator conflict policy. A focused editor claims its
/// own Ctrl+A/C/X/V and every unmodified printable/editing key; a modifier-
/// chorded binding (other Ctrl-chords, any Alt-chord) and every F-key win over
/// it. `(vk, ctrl, alt)` — the arguments the decision is computed from.
#[test]
fn editor_claims_matches_fixed_policy() {
    // Unmodified printable / editing keys → editor wins.
    assert!(editor_claims_key(VK_A, false, false), "unmodified letter is the editor's");
    assert!(editor_claims_key(VK_BACK, false, false), "Backspace is the editor's");
    assert!(editor_claims_key(VK_LEFT, false, false), "arrows are the editor's");

    // The editor's own clipboard / select chords → editor wins.
    assert!(editor_claims_key(VK_A, true, false), "Ctrl+A stays with the editor");
    assert!(editor_claims_key(VK_C, true, false), "Ctrl+C stays with the editor");
    assert!(editor_claims_key(VK_X, true, false), "Ctrl+X stays with the editor");
    assert!(editor_claims_key(VK_V, true, false), "Ctrl+V stays with the editor");

    // Any other chord, or an F-key → accelerator/system wins over the editor.
    assert!(!editor_claims_key(VK_S, true, false), "Ctrl+S is the app's to bind");
    assert!(!editor_claims_key(VK_A, false, true), "Alt+A is a chord, not editor input");
    assert!(!editor_claims_key(VK_F5, false, false), "F5 wins over the editor");
    assert!(
        !editor_claims_key(VK_A, true, true),
        "Ctrl+Alt+A (e.g. AltGr) is not an editor-claimed clipboard chord"
    );
}

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — this test asserts nothing without it; \
         run it in an interactive session",
    )
}

/// §7.3 bug 3 fixed: Escape in a NumberBox reverts the in-progress edit to the
/// last committed value and keeps that value unchanged — it does not commit the
/// typed text, and (verified by the untouched `ctrl().value`) fires no
/// ValueChanged. The committed value is reformatted to the field's precision.
#[test]
fn numberbox_escape_reverts_to_committed_value() {
    let mut a = harness();
    let nb = a.insert(ControlKind::NumberBox).expect("insert NumberBox");

    // Committed value 5.0 (default precision → 2 digits), then the user types a
    // new, uncommitted value into the buffer.
    a.ctrl_set_value(nb, 5.0);
    a.set_editor_text(nb, "9");
    assert_eq!(a.editor_text(nb).as_deref(), Some("9"), "the edit is in the buffer");

    // Escape discards it.
    a.number_escape_revert(nb);

    assert_eq!(
        a.editor_text(nb).as_deref(),
        Some("5.00"),
        "Escape must restore the committed value, formatted to precision"
    );
    assert_eq!(
        a.ctrl_probe(nb).map(|c| c.value),
        Some(5.0),
        "the committed value must not have changed (no ValueChanged)"
    );
}
