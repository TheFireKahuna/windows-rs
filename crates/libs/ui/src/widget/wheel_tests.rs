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
    let report = |target, notches| Report::Wheel { target, axis: WheelAxis::Rotate, notches };
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
    // The application's wheel handler takes the rotated wheel; a tilt is not its.
    out.clear();
    rig.tick(&[Report::Wheel { target: id, axis: WheelAxis::Tilt, notches: 1.0 }], &mut out)?;
    assert!(out.is_empty());
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

#[test]
fn native_wheel_reads_detents_per_axis_and_holds_a_burst_to_its_first_axis() -> Result<()> {
    use crate::seam::WheelOp;
    use crate::wheel::{WheelTable, px_per_notch};
    use windows_numerics::Vector2;
    use windows_scene::{SceneEvent, TrackerId};

    let mut rig = Rig::new("wheel table")?;
    let control = rig.ids.mint();
    let tracker = TrackerId::new(windows_scene::Ids::<{ windows_scene::TRACKER }>::default().mint());
    let mut table = WheelTable::default();
    table.apply(&mut vec![WheelOp::Add { tracker, control }]);
    let (rotate, tilt) = (px_per_notch(WheelAxis::Rotate), px_per_notch(WheelAxis::Tilt));
    let rest = |x: f32, y: f32| SceneEvent::InertiaBegan {
        tracker: tracker.erased(),
        rest: Vector2 { x, y },
        from_wheel: true,
    };
    let idle = SceneEvent::TrackerPhase { tracker: tracker.erased(), phase: windows_scene::Phase::Idle };
    let mut read = |events: &[SceneEvent]| -> Result<Vec<(WheelAxis, f32)>> {
        let mut out = Vec::new();
        rig.front(|front| table.reports(events, 2.0, &mut out, front))?;
        Ok(out
            .into_iter()
            .map(|report| match report {
                Report::Wheel { axis, notches, .. } => (axis, (notches * 1000.0).round() / 1000.0),
                _ => unreachable!("the table raises wheel reports only"),
            })
            .collect())
    };
    // A rotation away from the user lowers the position; a DIP is two pixels at this scale.
    assert_eq!(read(&[rest(0.0, -rotate)])?, [(WheelAxis::Rotate, 2.0)]);
    // A tilt in the same burst is dropped, and measured from where it left the tracker.
    assert_eq!(read(&[rest(tilt, -rotate)])?, []);
    assert_eq!(read(&[idle, rest(tilt * 1.25, -rotate)])?, [(WheelAxis::Tilt, 0.5)]);
    // Both axes in one rest: the larger detent count names the burst.
    assert_eq!(
        read(&[idle, rest(tilt * 1.5, -rotate * 2.0)])?,
        [(WheelAxis::Rotate, 2.0)]
    );
    Ok(())
}

#[test]
fn native_a_horizontal_slider_steps_on_the_tilted_wheel_only() -> Result<()> {
    let mut rig = Rig::new("slider tilt")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    let mut hit = entry(id, 0.0, 0.0, 126.0, 32.0);
    hit.flags = hit.flags | HitFlags::WHEEL;
    rig.publish_hits(&[hit])?;
    let row = scalar_tests::slider_row(thumb);
    let chrome = ChromeRow { flags: flag::SLIDE, ..ChromeRow::default() };
    rig.adopt(&[(id, chrome)], &[(id, row)], &[])?;
    let tilt = |notches| Report::Wheel { target: id, axis: WheelAxis::Tilt, notches };
    let values = |out: &[Intent]| -> Vec<(f64, bool)> {
        out.iter()
            .filter_map(|i| match i.what {
                What::Scalar { value, commit, .. } => Some(((value * 1e9).round() / 1e9, commit)),
                _ => None,
            })
            .collect()
    };
    let mut out = Vec::new();
    // One step of 4.8 a detent, moved and committed as one gesture, as a dial detent is.
    rig.tick(&[tilt(1.0)], &mut out)?;
    let moved = values(&out);
    assert_eq!(moved.last(), Some(&(4.8, true)));
    assert_eq!(moved.iter().filter(|(_, commit)| *commit).count(), 1);
    out.clear();
    rig.tick(&[tilt(-2.0)], &mut out)?;
    assert_eq!(values(&out).last(), Some(&(-4.8, true)));
    // The rotated wheel is not the slider's, and a vertical or disabled slider takes no tilt.
    out.clear();
    rig.tick(&[Report::Wheel { target: id, axis: WheelAxis::Rotate, notches: 1.0 }], &mut out)?;
    assert!(values(&out).is_empty());
    for flags in [flag::SLIDE | flag::VERTICAL, flag::SLIDE | flag::DISABLED] {
        rig.adopt(&[(id, ChromeRow { flags, ..chrome })], &[], &[])?;
        rig.tick(&[tilt(1.0)], &mut out)?;
        assert!(values(&out).is_empty());
    }
    Ok(())
}
