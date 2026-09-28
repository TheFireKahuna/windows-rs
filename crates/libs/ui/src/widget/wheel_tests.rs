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
    let report = |target, notches, horizontal| Report::Wheel {
        target, notches, horizontal, at: windows_scene::Point::default(),
    };
    let mut out = Vec::new();
    rig.tick(&[report(Some(id), 1.0, false)], &mut out)?;
    assert!(out.is_empty());
    hit.flags = hit.flags | HitFlags::WHEEL;
    rig.publish_hits(&[hit])?;
    for (notches, horizontal) in [(0.25, false), (-2.0, false), (1.0, true)] {
        out.clear();
        rig.tick(&[report(Some(id), notches, horizontal)], &mut out)?;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, id);
        assert_eq!(out[0].what, What::Wheel { notches, horizontal });
    }
    out.clear();
    for notches in [0.0, f32::NAN, f32::INFINITY] {
        rig.tick(&[report(Some(id), notches, false)], &mut out)?;
    }
    rig.tick(&[report(None, 1.0, false)], &mut out)?;
    assert!(out.is_empty());
    rig.adopt(&[(id, ChromeRow { flags: flag::DRAGS, ..row })], &[], &[])?;
    rig.tick(&[scalar_tests::press(id), Report::Dragged {
        target: id, contact: 1,
        update: crate::gesture::DragUpdate {
            phase: crate::gesture::Phase::Free, decided: true,
            from: windows_scene::Point::default(), at: windows_scene::Point { x: 12.0, y: 0.0 },
            delta: windows_scene::Point { x: 12.0, y: 0.0 },
        },
    }], &mut out)?;
    out.clear();
    rig.tick(&[report(Some(id), 1.0, false)], &mut out)?;
    assert!(out.is_empty());
    rig.tick(&[Report::Canceled { target: id, contact: 1 }], &mut out)?;
    out.clear();
    rig.tick(&[report(Some(id), 1.0, false)], &mut out)?;
    assert_eq!(out.last().map(|i| i.what), Some(What::Wheel { notches: 1.0, horizontal: false }));
    out.clear();
    rig.adopt(&[(id, ChromeRow { flags: flag::DISABLED, ..row })], &[], &[])?;
    rig.tick(&[report(Some(id), 1.0, false)], &mut out)?;
    assert!(out.is_empty());
    rig.adopt(&[], &[], &[id])?;
    rig.tick(&[report(Some(id), 1.0, false)], &mut out)?;
    assert!(out.is_empty());
    Ok(())
}
