use super::*;
use super::scalar_tests::{Rig, entry};

#[test]
fn native_wheel_preserves_fractional_detents_and_requires_a_live_opted_in_target() -> Result<()> {
    let mut rig = Rig::new("wheel callback")?;
    let id = rig.ids.mint();
    let mut hit = entry(id, 0.0, 0.0, 100.0, 100.0);
    let row = ChromeRow::default();
    rig.adopt(&[(id, row)], &[], &[])?;
    rig.publish_hits(&[hit])?;
    let report = |target, notches| Report::Wheel { target, notches };
    let mut out = Vec::new();
    rig.tick(&[report(id, 1.0)], &mut out)?;
    assert!(out.is_empty());
    hit.flags = hit.flags | HitFlags::WHEEL;
    rig.publish_hits(&[hit])?;
    for notches in [0.25, -2.0, 1.0] {
        out.clear();
        rig.tick(&[report(id, notches)], &mut out)?;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, id);
        assert_eq!(out[0].what, What::Wheel { notches });
    }
    out.clear();
    for notches in [0.0, f32::NAN, f32::INFINITY] {
        rig.tick(&[report(id, notches)], &mut out)?;
    }
    assert!(out.is_empty());
    rig.adopt(&[(id, ChromeRow { flags: flag::DRAGS, ..row })], &[], &[])?;
    rig.tick(&[scalar_tests::press(id), Report::Dragged {
        target: id, contact: 1,
        update: DragUpdate {
            phase: Phase::Free, decided: true,
            from: Point::default(), at: Point { x: 12.0, y: 0.0 },
            delta: Point { x: 12.0, y: 0.0 },
        },
    }], &mut out)?;
    out.clear();
    rig.tick(&[report(id, 1.0)], &mut out)?;
    assert!(out.is_empty());
    rig.tick(&[Report::Canceled { target: id, contact: 1 }], &mut out)?;
    out.clear();
    rig.tick(&[report(id, 1.0)], &mut out)?;
    assert_eq!(out.last().map(|i| i.what), Some(What::Wheel { notches: 1.0 }));
    out.clear();
    rig.adopt(&[(id, ChromeRow { flags: flag::DISABLED, ..row })], &[], &[])?;
    rig.tick(&[report(id, 1.0)], &mut out)?;
    assert!(out.is_empty());
    rig.adopt(&[], &[], &[id])?;
    rig.tick(&[report(id, 1.0)], &mut out)?;
    assert!(out.is_empty());
    Ok(())
}
