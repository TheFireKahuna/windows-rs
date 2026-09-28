use super::*;
use super::scalar_tests::{Rig, entry};

#[test]
fn native_double_tap_uses_current_local_coordinates_and_declines_child_hits() -> Result<()> {
    let mut rig = Rig::new("double tap callback")?;
    let id = rig.ids.mint();
    let child = rig.ids.mint();
    let row = ChromeRow::default();
    rig.adopt(&[(id, row), (child, row)], &[], &[])?;
    let report = |at, count| Report::Gesture {
        target: id, contact: 1, event: Recognised::Tapped { at, count },
    };
    let mut out = Vec::new();
    for (x, y, w, h) in [(80.0, 50.0, 400.0, 200.0), (20.0, 100.0, 700.0, 300.0)] {
        rig.publish_hits(&[entry(id, x, y, x + w, y + h)])?;
        let at = Point { x: x + w * 0.5, y: y + h * 0.25 };
        for count in [1, 3] {
            out.clear();
            rig.tick(&[report(at, count)], &mut out)?;
            assert!(out.is_empty());
        }
        out.clear();
        rig.tick(&[report(at, 1), report(at, 2)], &mut out)?;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].what, What::DoubleTapped(Point { x: w * 0.5, y: h * 0.25 }));
        rig.publish_hits(&[entry(id, x, y, x + w, y + h),
            entry(child, at.x - 10.0, at.y - 10.0, at.x + 10.0, at.y + 10.0)])?;
        out.clear();
        rig.tick(&[report(at, 2)], &mut out)?;
        assert!(out.is_empty());
        rig.tick(&[report(Point { x: x + 1.0, y: y + 1.0 }, 1), Report::Gesture {
            target: child, contact: 1, event: Recognised::Tapped { at, count: 2 },
        }], &mut out)?;
        assert!(out.is_empty(), "two different targets cannot combine into a double tap");
        rig.tick(&[report(Point { x: -1.0, y: -1.0 }, 2),
            report(Point { x: f32::NAN, y: 0.0 }, 2)], &mut out)?;
        assert!(out.is_empty());
    }
    rig.publish_hits(&[entry(id, 0.0, 0.0, 400.0, 200.0)])?;
    let at = Point { x: 100.0, y: 100.0 };
    rig.tick(&[report(at, 1), Report::CaptureLost, report(at, 2)], &mut out)?;
    assert!(out.is_empty());
    for cancel in [Report::FocusChanged { from: Some(id), to: None },
        Report::Dismiss { blocker: child, scope: None }]
    {
        rig.tick(&[report(at, 1), cancel, report(at, 2)], &mut out)?;
        assert!(out.is_empty());
    }
    rig.adopt(&[(id, ChromeRow { flags: flag::DISABLED, ..row })], &[], &[])?;
    rig.tick(&[report(at, 2)], &mut out)?;
    assert!(out.is_empty());
    rig.adopt(&[], &[], &[id])?;
    assert!(rig.controls.tapped.is_none());
    rig.tick(&[report(at, 2)], &mut out)?;
    assert!(out.is_empty());
    Ok(())
}
