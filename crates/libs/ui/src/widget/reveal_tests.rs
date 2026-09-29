//! The interaction reveal and the observed-hover edges, driven against a real compositor.

use super::scalar_tests::{Rig, entry, press};
use super::*;
use windows_scene::Point;

fn hover(from: ControlId, to: ControlId) -> Report {
    Report::HoverChanged {
        from: (!from.is_none()).then_some(from),
        to: (!to.is_none()).then_some(to),
        at: Point::default(),
        qpc: 0,
    }
}

#[test]
fn native_focus_border_survives_pointer_press_and_releases_without_idle_work() -> Result<()> {
    let mut rig = Rig::new("focus border")?;
    let (a, b) = (rig.ids.mint(), rig.ids.mint());
    let (wa, wb) = (SpriteId(rig.node()?), SpriteId(rig.node()?));
    let row = |wash| ChromeRow { wash, hover: 0.08, press: 0.12,
        flags: flag::FOCUS_WASH, ..ChromeRow::default() };
    rig.adopt(&[(a, row(wa)), (b, row(wb))], &[], &[])?;
    let mut out = Vec::with_capacity(16);
    rig.tick(&[Report::FocusChanged { from: None, to: Some(a) }], &mut out)?;
    assert_eq!(rig.controls.wash_target(a), Some((wa, 1.0)));
    rig.tick(&[press(a)], &mut out)?;
    assert!(rig.controls.focused.is_none());
    assert_eq!(rig.controls.input_focus, a);
    assert_eq!(rig.controls.wash_target(a), Some((wa, 1.0)));
    rig.tick(&[Report::Canceled { target: a, contact: 1 }], &mut out)?;
    assert!(out.iter().all(|intent| matches!(intent.what, What::Canceled(_))));
    out.clear();
    rig.tick(&[Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
    assert_eq!(rig.controls.wash_target(a), Some((wa, 0.0)));
    assert_eq!(rig.controls.wash_target(b), Some((wb, 1.0)));
    rig.tick(&[hover(ControlId::NONE, b)], &mut out)?;
    rig.adopt(&[(b, ChromeRow { flags: flag::DISABLED, ..row(wb) })], &[], &[])?;
    assert_eq!(rig.controls.wash_target(b), Some((wb, 0.0)));
    rig.adopt(&[(b, row(wb))], &[], &[])?;
    assert_eq!(rig.controls.wash_target(b), Some((wb, 1.0)));
    rig.tick(&[hover(b, ControlId::NONE)], &mut out)?;
    let minted = rig.visuals_minted();
    for _ in 0..2 {
        rig.tick(&[Report::FocusChanged { from: Some(b), to: Some(a) },
            Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
    }
    let allocations = crate::counting::allocations();
    for _ in 0..100 {
        rig.tick(&[Report::FocusChanged { from: Some(b), to: Some(a) },
            Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
    }
    assert_eq!(crate::counting::allocations(), allocations);
    assert_eq!(rig.visuals_minted(), minted);
    let settled = rig.animations();
    rig.tick(&[], &mut out)?;
    assert_eq!(rig.animations(), settled);
    assert!(out.is_empty());
    rig.adopt(&[], &[], &[b])?;
    assert!(rig.controls.input_focus.is_none());
    assert_eq!(rig.controls.wash_target(b), None);
    rig.tick(&[Report::FocusChanged { from: None, to: Some(b) }], &mut out)?;
    assert!(rig.controls.input_focus.is_none());
    Ok(())
}

#[test]
fn native_focus_outline_follows_keyboard_focus_and_hides_for_pointer_and_retirement() -> Result<()> {
    let mut rig = Rig::new("focus outline")?;
    let ring = rig.overlay_node()?;
    rig.controls.set_ring(ring);
    let (a, b) = (rig.ids.mint(), rig.ids.mint());
    rig.publish_hits(&[entry(a, 10.0, 20.0, 100.0, 32.0), entry(b, 120.0, 20.0, 100.0, 32.0)])?;
    rig.adopt(&[(a, ChromeRow::default()), (b, ChromeRow::default())], &[], &[])?;
    let minted = rig.visuals_minted();
    let mut out = Vec::with_capacity(16);
    rig.tick(&[Report::FocusChanged { from: None, to: Some(a) }], &mut out)?;
    assert!(rig.controls.ring_shown);
    rig.tick(&[Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
    assert!(rig.controls.ring_shown);
    rig.tick(&[press(b)], &mut out)?;
    assert!(!rig.controls.ring_shown);
    rig.tick(&[Report::FocusChanged { from: Some(b), to: Some(a) }], &mut out)?;
    assert!(rig.controls.ring_shown);
    rig.tick(&[Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
    rig.tick(&[Report::FocusChanged { from: Some(b), to: Some(a) }], &mut out)?;
    let before = crate::counting::allocations();
    for i in 0..100 {
        let at = crate::counting::allocations();
        rig.tick(&[Report::FocusChanged { from: Some(a), to: Some(b) }], &mut out)?;
        rig.tick(&[Report::FocusChanged { from: Some(b), to: Some(a) }], &mut out)?;
        let rose = crate::counting::allocations() - at;
        if rose != 0 {
            println!("iteration {i}: +{rose} allocations");
        }
    }
    assert_eq!(crate::counting::allocations(), before, "warm focus moves allocate no Rust storage");
    let settled = rig.animations();
    rig.tick(&[], &mut out)?;
    assert_eq!(rig.animations(), settled);
    assert!(out.is_empty(), "focus appearance emits no application intent");
    rig.adopt(&[], &[], &[a])?;
    assert!(!rig.controls.ring_shown);
    assert_eq!(rig.visuals_minted(), minted);
    Ok(())
}

/// A scope carrying a reveal target, and two children that belong to it.
struct Scoped {
    scope: ControlId,
    a: ControlId,
    b: ControlId,
}

fn scoped(rig: &mut Rig, observes: bool) -> Result<Scoped> {
    let reveal = rig.node()?;
    let washes = [SpriteId(rig.node()?), SpriteId(rig.node()?)];
    let (scope, a, b) = (rig.ids.mint(), rig.ids.mint(), rig.ids.mint());
    let child = |wash| ChromeRow {
        wash,
        scope,
        hover: 0.08,
        press: 0.16,
        ..ChromeRow::default()
    };
    rig.publish_hits(&[
        entry(scope, 0.0, 0.0, 200.0, 32.0),
        entry(a, 0.0, 0.0, 100.0, 32.0),
        entry(b, 100.0, 0.0, 200.0, 32.0),
    ])?;
    rig.adopt(
        &[
            (
                scope,
                ChromeRow {
                    reveal,
                    scope,
                    flags: if observes { flag::OBSERVES } else { 0 },
                    ..ChromeRow::default()
                },
            ),
            (a, child(washes[0])),
            (b, child(washes[1])),
        ],
        &[],
        &[],
    )?;
    Ok(Scoped { scope, a, b })
}

#[test]
fn native_a_reveal_survives_crossings_between_the_children_of_one_scope() -> Result<()> {
    let mut rig = Rig::new("interaction reveal")?;
    let Scoped { scope, a, b } = scoped(&mut rig, false)?;
    let mut out = Vec::with_capacity(16);
    let minted = rig.visuals_minted();

    let before = rig.animations();
    rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
    assert!(
        out.is_empty(),
        "a cosmetic hover must not wake the application"
    );
    // One wash and one reveal: the target came up and the control under the pointer lit.
    assert_eq!(rig.animations() - before, 2);

    let settled = rig.animations();
    for _ in 0..16 {
        rig.tick(&[hover(a, b), hover(b, a)], &mut out)?;
    }
    assert_eq!(
        rig.animations() - settled,
        64,
        "crossing children must animate only their existing washes"
    );
    assert_eq!(
        rig.visuals_minted(),
        minted,
        "an interaction mints no visual"
    );
    assert!(out.is_empty());

    // Keyboard focus keeps the actions visible while the pointer leaves.
    rig.tick(
        &[
            Report::FocusChanged {
                from: None,
                to: Some(b),
            },
            hover(a, ControlId::NONE),
        ],
        &mut out,
    )?;
    let held = rig.animations();
    rig.tick(&[], &mut out)?;
    assert_eq!(
        rig.animations(),
        held,
        "an unchanged reveal is not restarted"
    );

    // Focus leaves and nothing is left holding the scope, so the target fades.
    rig.tick(
        &[Report::FocusChanged {
            from: Some(b),
            to: None,
        }],
        &mut out,
    )?;
    assert!(rig.animations() > held);

    // A touch press reveals without a hover, and a canceled contact takes it back down.
    let up = rig.animations();
    rig.tick(&[press(b)], &mut out)?;
    assert!(rig.animations() > up);
    let down = rig.animations();
    rig.tick(
        &[Report::Canceled {
            target: b,
            contact: 1,
        }],
        &mut out,
    )?;
    assert!(rig.animations() > down);

    // The scope itself retires: the target is gone, so nothing may be written to it.
    rig.adopt(&[], &[], &[scope])?;
    let retired = rig.animations();
    rig.tick(&[hover(ControlId::NONE, a), hover(a, b)], &mut out)?;
    assert_eq!(
        rig.animations() - retired,
        3,
        "only the two washes move once the reveal target is gone"
    );
    Ok(())
}

#[test]
fn native_control_reveal_keeps_its_enclosing_scope_lit() -> Result<()> {
    let mut rig = Rig::new("control and scope reveal")?;
    let Scoped { scope, a, b } = scoped(&mut rig, false)?;
    let reveal = rig.node()?;
    let mut row = rig.controls.chrome_of(a);
    row.reveal = reveal;
    rig.adopt(&[(a, row)], &[], &[])?;
    let minted = rig.visuals_minted();
    let before = rig.animations();
    let mut out = Vec::with_capacity(16);
    rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
    assert_eq!(rig.animations() - before, 3, "wash, control and enclosing scope");
    assert!(rig.controls.revealed.contains(&a) && rig.controls.revealed.contains(&scope));
    let before = rig.animations();
    rig.tick(&[hover(a, b)], &mut out)?;
    assert_eq!(rig.animations() - before, 3, "two washes and the control reveal only");
    assert!(!rig.controls.revealed.contains(&a) && rig.controls.revealed.contains(&scope));
    rig.tick(&[Report::FocusChanged { from: None, to: Some(a) }, hover(b, ControlId::NONE)], &mut out)?;
    assert!(rig.controls.revealed.contains(&a) && rig.controls.revealed.contains(&scope));
    let before = rig.animations();
    rig.tick(&[], &mut out)?;
    assert_eq!(rig.animations(), before);
    assert_eq!(rig.visuals_minted(), minted);
    assert!(out.is_empty(), "cosmetic interaction must not wake the application");
    rig.adopt(&[], &[], &[a])?;
    assert!(!rig.controls.revealed.contains(&a));
    Ok(())
}

#[test]
fn native_semantic_hover_reports_scope_edges_and_ignores_child_crossings() -> Result<()> {
    let mut rig = Rig::new("observed hover")?;
    let Scoped { scope, a, b } = scoped(&mut rig, true)?;
    let ordinary = rig.ids.mint();
    rig.adopt(&[(ordinary, ChromeRow::default())], &[], &[])?;

    let mut out = Vec::with_capacity(2);
    rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
    assert_eq!(
        out,
        [Intent {
            target: scope,
            what: What::Hovered(true)
        }]
    );
    out.clear();

    for _ in 0..1000 {
        rig.tick(&[hover(a, b), hover(b, a)], &mut out)?;
    }
    assert!(out.is_empty(), "child crossings must stay scene-side");

    rig.tick(&[hover(a, ordinary)], &mut out)?;
    assert_eq!(
        out,
        [Intent {
            target: scope,
            what: What::Hovered(false)
        }]
    );
    out.clear();

    rig.tick(&[hover(ordinary, ControlId::NONE)], &mut out)?;
    assert!(
        out.is_empty(),
        "leaving a control outside a scope is not an edge"
    );
    assert_eq!(
        out.capacity(),
        2,
        "the edge path appends and never allocates"
    );
    Ok(())
}

#[test]
fn native_translation_keeps_child_crossings_quiet_and_retires_shared_geometry() -> Result<()> {
    let mut rig = Rig::new("interaction translation")?;
    let node = rig.node()?;
    let (scope, a, b) = (rig.ids.mint(), rig.ids.mint(), rig.ids.mint());
    let mut first = entry(a, 10.0, 10.0, 40.0, 40.0);
    first.parent = 0;
    let mut second = entry(b, 50.0, 10.0, 90.0, 40.0);
    second.parent = 0;
    rig.publish_hits(&[entry(scope, 0.0, 0.0, 100.0, 50.0), first, second])?;
    rig.adopt(&[(scope, ChromeRow::default()), (a, ChromeRow::default()),
        (b, ChromeRow::default())], &[], &[])?;
    let state = windows_scene::Translation::new(Vector2::new(0.0, -3.0));
    rig.adopt_translations(&[(scope, node, state.clone())], &[])?;
    let mut out = Vec::with_capacity(8);
    let before = rig.animations();
    rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
    assert_eq!(state.get().y, -3.0);
    assert!(rig.controls.take_translation_changed());
    assert_eq!(rig.animations(), before + 1);
    rig.tick(&[hover(a, b)], &mut out)?;
    assert_eq!(rig.animations(), before + 1);
    assert!(!rig.controls.take_translation_changed());
    rig.tick(&[Report::FocusChanged { from: None, to: Some(a) }, hover(b, ControlId::NONE)], &mut out)?;
    assert_eq!(state.get().y, -3.0);
    rig.tick(&[Report::FocusChanged { from: Some(a), to: None }], &mut out)?;
    assert_eq!(state.get(), Vector2::zero());
    for _ in 0..2 {
        rig.tick(&[hover(ControlId::NONE, a), hover(a, ControlId::NONE)], &mut out)?;
    }
    let allocations = crate::counting::allocations();
    for _ in 0..100 {
        rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
        rig.tick(&[hover(a, ControlId::NONE)], &mut out)?;
    }
    assert_eq!(crate::counting::allocations(), allocations);
    assert!(out.is_empty());
    let settled = rig.animations();
    rig.tick(&[], &mut out)?;
    assert_eq!(rig.animations(), settled);
    rig.tick(&[hover(ControlId::NONE, a)], &mut out)?;
    rig.adopt(&[], &[], &[scope])?;
    rig.adopt_translations(&[], &[scope])?;
    assert_eq!(state.get(), Vector2::zero());
    assert!(rig.controls.translations.is_empty());
    Ok(())
}
