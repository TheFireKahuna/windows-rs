use super::*;
use crate::build::{Ui, rig::fixture_at};
use crate::layout::{Len, Preset};
use crate::signal::{Cell, Owner};
use windows_scene::Op;

#[test]
fn dip_entrance_preserves_scale_and_completes_once_at_each_dpi() {
    for dpi in [96.0, 144.0, 192.0] {
        let mut patch = fixture_at(dpi);
        let (_owner, mount) = Owner::scope(|| Ui::mount_root(|ui| {
            ui.node(Preset::Stack).width(Len::dip(200.0)).height(Len::dip(100.0))
                .enter_from(Vector2::new(0.0, 8.0), 350, 100, Easing::Linear);
        }));
        Host::flush(&mut patch);
        let (node, frames) = patch.ops().iter().find_map(|op| match *op {
            Op::Bind { id, prop: Prop::AnchorY, bind: Bind::Animate(Anim::Frames { frames, duration_ms: 450, .. }) } => Some((id, frames)),
            _ => None,
        }).expect("translation");
        let keys = patch.frames(frames);
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].1, Value::Scalar(-0.08));
        assert_eq!(keys[1].0, 100.0 / 450.0);
        assert_eq!(keys[1].1, keys[0].1);
        assert_eq!(keys[2].1, Value::Scalar(0.0));
        assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Bind { id, prop: Prop::Opacity, bind: Bind::Animate(Anim::Frames { duration_ms: 450, .. }) } if *id == node)));
        assert!(!patch.ops().iter().any(|op| matches!(op,
            Op::Bind { prop: Prop::Scale | Prop::ScaleX | Prop::ScaleY, .. })));
        assert!(Host::with(|h| h.input_suspended(node)));
        for width in [700.0, 800.0] {
            patch.clear();
            Host::with(|h| h.set_window(Vector2::new(width, 600.0)));
            Host::flush(&mut patch);
            assert!(!patch.ops().iter().any(|op| matches!(op,
                Op::Bind { id, prop: Prop::AnchorY | Prop::Opacity, .. } if *id == node)));
        }
        Host::with(|h| h.complete_overlay_entry(node));
        assert!(!Host::with(|h| h.input_suspended(node)));
        patch.clear();
        Host::flush(&mut patch);
        patch.clear();
        let before = crate::counting::allocations();
        for _ in 0..20 { Host::flush(&mut patch); }
        assert_eq!(crate::counting::allocations(), before);
        assert!(patch.ops().is_empty());
        drop(mount);
        Host::flush(&mut patch);
        assert!(Host::with(|h| h.entrances.is_empty()));
    }
}

#[test]
fn dip_entrance_waits_for_size_and_retires_before_completion() {
    let mut patch = fixture_at(144.0);
    let shown = Cell::new(true);
    let height = Cell::new(0.0);
    let (_owner, _mount) = Owner::scope(|| Ui::mount_root(|ui| {
        ui.when(shown, move |ui| {
            ui.node(Preset::Stack).width(Len::dip(200.0))
                .layout_from(move |layout| layout.height = Len::dip(height.get()))
                .enter_from(Vector2::new(0.0, 8.0), 350, 0, Easing::Linear);
        });
    }));
    Host::flush(&mut patch);
    assert!(!patch.ops().iter().any(|op| matches!(op,
        Op::Bind { prop: Prop::AnchorY, bind: Bind::Animate(_), .. })));
    height.set(100.0);
    patch.clear();
    Host::flush(&mut patch);
    let node = Host::with(|h| {
        assert!(h.entrances[0].started);
        h.entrances[0].node
    });
    shown.set(false);
    patch.clear();
    Host::flush(&mut patch);
    Host::with(|h| {
        assert!(h.entrances.is_empty());
        assert!(!h.tree.is_live(node));
        h.complete_overlay_entry(node);
    });
}
