//! `backend/dcomp/node.rs` — the lazily-boxed `Ctrl`.
//!
//! `Ctrl` is the largest per-node payload after the Taffy style (512 bytes of a
//! 1800-byte `Node`), yet a real tree is overwhelmingly `TextBlock` / `Border` /
//! `Grid` / `StackPanel` — kinds that hold none of it. It is therefore boxed and
//! allocated on first write.
//!
//! That is only safe if a node with NO allocated `Ctrl` reads exactly what an
//! eagerly-constructed one would have held. `Ctrl`'s defaults are deliberately
//! not all-zero (`max: 100.0`, `is_active: true`, `selected_index: -1`,
//! `hot_index: -1`, `content_align: -1`), so getting this wrong would not crash
//! — it would silently change what every drawn control paints. These tests pin
//! the equivalence rather than trusting it.
//!
//! **These tests are not headless** — see `arena_ids.rs` for why the harness
//! needs a windowless compositor, and why it fails loudly without one.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::ControlKind;

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

/// A freshly created node carries no `Ctrl` allocation. This is the whole point
/// of the box: if every node materialised one on construction the indirection
/// would cost a pointer chase and buy nothing.
#[test]
fn a_new_node_allocates_no_ctrl() {
    let mut a = harness();
    for kind in [
        ControlKind::StackPanel,
        ControlKind::Grid,
        ControlKind::Border,
        ControlKind::TextBlock,
        // Even the stateful kinds start empty — state arrives via `set_prop`.
        ControlKind::Slider,
        ControlKind::ToggleSwitch,
    ] {
        let id = a.insert(kind).unwrap();
        assert_eq!(
            a.ctrl_allocated(id),
            Some(false),
            "{kind:?} allocated a Ctrl at construction"
        );
    }
}

/// Reading an ABSENT `Ctrl` returns exactly what a materialised-but-untouched
/// one returns — including every field whose default is not zero.
///
/// This is the invariant the whole change rests on. Both sides come from the
/// single `Ctrl::DEFAULT` constant, so they cannot drift; this test is what
/// notices if someone gives the two paths separate definitions again.
#[test]
fn an_absent_ctrl_reads_as_a_default_one() {
    let mut a = harness();

    let absent = a.insert(ControlKind::StackPanel).unwrap();
    let present = a.insert(ControlKind::StackPanel).unwrap();
    a.ctrl_materialize(present);

    assert_eq!(a.ctrl_allocated(absent), Some(false));
    assert_eq!(a.ctrl_allocated(present), Some(true));
    assert_eq!(
        a.ctrl_probe(absent),
        a.ctrl_probe(present),
        "a node with no allocated Ctrl read differently from one with an \
         untouched Ctrl — every drawn control's paint depends on these values"
    );

    // Pin the non-zero defaults literally, so a future edit to `Ctrl::DEFAULT`
    // that changes behaviour cannot pass by changing both sides at once.
    let p = a.ctrl_probe(absent).unwrap();
    assert_eq!(p.max, 100.0, "max defaults to 100, not 0");
    assert!(p.is_active, "is_active defaults to true, not false");
    assert_eq!(p.selected_index, -1, "selected_index defaults to -1");
    assert_eq!(p.hot_index, -1, "hot_index defaults to -1");
    assert_eq!(p.content_align, -1, "content_align defaults to -1");
    assert_eq!(p.min, 0.0);
    assert_eq!(p.value, 0.0);
    assert!(!p.is_on);
    assert_eq!(p.items, 0);
    assert_eq!(p.menu, 0);
    assert_eq!(p.placeholder, "");
}

/// A write materialises the box, and only the written field moves — the rest of
/// the node still reads the defaults it read while absent.
#[test]
fn a_write_materializes_the_ctrl_and_leaves_the_rest_default() {
    let mut a = harness();
    let id = a.insert(ControlKind::ToggleSwitch).unwrap();

    let before = a.ctrl_probe(id).unwrap();
    assert!(!before.is_on);
    assert_eq!(a.ctrl_allocated(id), Some(false));

    a.ctrl_set_is_on(id, true);

    assert_eq!(a.ctrl_allocated(id), Some(true), "the write must allocate");
    let after = a.ctrl_probe(id).unwrap();
    assert!(after.is_on, "the written field took the value");
    assert_eq!(
        CtrlProbeRest::of(&after),
        CtrlProbeRest::of(&before),
        "materialising the Ctrl disturbed a field the write never touched"
    );
}

/// Everything in a probe except the field the write above targets.
#[derive(Debug, PartialEq)]
struct CtrlProbeRest {
    value: f64,
    min: f64,
    max: f64,
    is_active: bool,
    selected_index: i32,
    hot_index: i32,
    content_align: i32,
    items: usize,
    menu: usize,
    placeholder: String,
}

impl CtrlProbeRest {
    fn of(p: &windows_reactor::dcomp_test_api::CtrlProbe) -> Self {
        Self {
            value: p.value,
            min: p.min,
            max: p.max,
            is_active: p.is_active,
            selected_index: p.selected_index,
            hot_index: p.hot_index,
            content_align: p.content_align,
            items: p.items,
            menu: p.menu,
            placeholder: p.placeholder.clone(),
        }
    }
}
