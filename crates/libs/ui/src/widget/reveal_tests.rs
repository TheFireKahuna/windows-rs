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
