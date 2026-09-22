//! The arena's claims: the host and its one patch, the builder, bindings, retirement, the
//! encode, and the one walk that fills the hit array and the automation rows.
//!
//! The claims, and where each comes from.
//!
//! The host and its one patch
//!  1. A flush hands over what moved, and a settled tree hands over nothing. [00-ARCHITECTURE §8]
//!  2. A patch is stamped with the environment it was solved under. [00-ARCHITECTURE §8]
//!
//! The builder
//!  3. Children are ordered bottom-first, and a later sibling sits above. [03-LAYOUT §7]
//!  3a. A plate declared before the body stays beneath the body's first child. [03-LAYOUT §7]
//!  3b. A node given no setter is measured on its first solve. [03-LAYOUT §3]
//!  3c. A subtree hidden at mount takes its size when shown. [03-LAYOUT §3]
//!  3d. A hidden subtree's derived sprites take no pixels. [03-LAYOUT §3]
//!  4. A container's closure runs with that container as the parent. [09-AUTHORING §1]
//!  5. Editing through an expired handle answers absence. [09-AUTHORING §2]
//!  6. A constant false branch creates no content. [09-AUTHORING §5]
//!  7. Two adjacent branches keep their order across being empty. [09-AUTHORING §5]
//!  8. A keyed list moves survivors rather than reminting them. [09-AUTHORING §5]
//!
//! Bindings
//!  9. A constant writes the record and installs no writer. [09-AUTHORING §3]
//! 10. A reactive source writes once per distinct value. [09-AUTHORING §3]
//! 11. Repeating a singleton handler replaces it. [09-AUTHORING §2]
//! 12. A geometry channel snaps and a chrome channel springs. [17-WIDGETS §8.2]
//!
//! Mount and retirement
//! 13. Retiring a subtree is one op per root and reclaims every id. [09-AUTHORING §5]
//! 14. Unmounting releases every row the subtree claimed. [09-AUTHORING §5]
//!
//! The encode
//! 15. A box that moved is published and one that did not is not. [03-LAYOUT §7]
//! 16. A node carried by its parent restates no offset of its own. [03-LAYOUT §7]
//!
//! The one hit array
//! 17. The array is paint order and the id index is id order. [03-LAYOUT §7]
//! 18. Touch inflation never lets two targets claim one point. [03-LAYOUT §7]
//! 19. A scroll rail declares a control and no handler row. [17-WIDGETS §8.4]
//!
//! The automation rows the same walk emits
//! 20. A control is named by the text its subtree laid out. [09-AUTHORING §6]
//! 21. An element with no role is skipped and its children reparent past it. [09-AUTHORING §6]
//! 22. A disabled control keeps its entry and states that it is disabled. [17-WIDGETS §8.1]
//! 23. A slider publishes the range it runs over. [09-AUTHORING §6]
//! 24. A live region states how it announces. [14-OVERLAYS §10, 09-AUTHORING §6]
//! 25. A selected control states that it selects. [17-WIDGETS §8.1]

use super::host::Host;
use super::rig::{Kind, Rig};
use super::ui::Ui;
use crate::layout::{Align, Len, Preset, scroll};
use crate::role::{Fill, Metric, Role};
use crate::signal::Cell;
use crate::uia::{ColFlags, State};
use crate::widget::{
    Intent, Range, UiaRole, What, box_, button, caption, knob, label, micro, text,
};
use windows_numerics::Vector2;
use windows_scene::{Anim, Bind, ContactKind, ControlId, NodeId, Op, Prop, Value};

pub(crate) use super::rig::fixture;

/// A node of a stated size, which is the fixture most claims below need.
fn boxed(ui: &mut Ui<'_>, w: f32, h: f32) -> NodeId {
    ui.node(Preset::Layer)
        .width(Len::dip(w))
        .height(Len::dip(h))
        .id()
        .into()
}

/// The centre of `node`'s solved box, which is where a contact reaches it.
fn centre(node: NodeId) -> (f32, f32) {
    let box_ = Host::with(|h| h.geom(node).rect);
    ((box_.x0 + box_.x1) * 0.5, (box_.y0 + box_.y1) * 0.5)
}

// ── the host and its one patch ──────────────────────────────────────────────────────

#[test]
fn a_flush_hands_over_what_moved_and_a_settled_tree_hands_over_nothing() {
    let mut patch = fixture();
    let held = Ui::mount_root(|ui| {
        boxed(ui, 100.0, 40.0);
    });
    Host::flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(op, Op::New { .. })));
    patch.clear();
    Host::flush(&mut patch);
    assert!(patch.ops().is_empty(), "a settled tree published work");
    drop(held);
}

#[test]
fn layout_width_changes_animate_the_retained_row_and_its_clip() {
    let mut patch = fixture();
    let width = Cell::new(240.0);
    let (mut body, mut pane) = (NodeId::NONE, NodeId::NONE);
    let held = Ui::mount_root(|ui| {
        ui.node(Preset::Row).animate_layout().width(Len::dip(800.0)).height(Len::dip(400.0))
            .children(|ui| {
                body = ui.node(Preset::Layer).grow().id().into();
                pane = ui.node(Preset::Layer).clip()
                    .layout_from(move |l| l.width = Len::dip(width.get()))
                    .children(|ui| { button(ui, "Pane action"); })
                    .id().into();
            });
    });
    Host::flush(&mut patch);
    assert!(!patch.ops().iter().any(|op| matches!(op, Op::Bind { bind: Bind::Animate(_), .. })));
    assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Clip { id, clip: windows_scene::Clip::Bounds } if *id == pane
    )));
    for target in [0.0, 240.0, 180.0, 0.0] {
        patch.clear();
        Host::with(|h| h.uia_published());
        width.set(target);
        Host::flush(&mut patch);
        assert!(Host::with(|h| h.uia_stale()));
        for (node, prop) in [(body, Prop::Size), (pane, Prop::Offset), (pane, Prop::Size)] {
            assert!(patch.ops().iter().any(|op| matches!(op,
                Op::Bind { id, prop: p, bind: Bind::Animate(Anim::Spring { tuning: windows_scene::Tuning::Layout, .. }) }
                if *id == node && *p == prop
            )), "missing native {prop:?} animation: {:#?}", patch.ops());
        }
        assert!(!patch.ops().iter().any(|op| matches!(op, Op::New { .. } | Op::Drop { .. })));
        assert_eq!(Host::with(|h| h.geom(body).size.x), 800.0 - target);
        patch.clear();
        Host::flush(&mut patch);
        assert!(patch.ops().is_empty());
    }
    drop(held);
}

#[test]
fn a_window_resize_sets_animated_layout_bounds_directly() {
    let mut patch = fixture();
    Host::with(|h| h.set_window(Vector2::new(800.0, 600.0)));
    let width = Cell::new(240.0);
    let mut pane = NodeId::NONE;
    let held = Ui::mount_root(|ui| {
        ui.node(Preset::Row).animate_layout().grow().children(|ui| {
            ui.node(Preset::Layer).grow();
            pane = ui.node(Preset::Layer).layout_from(move |l| l.width = Len::dip(width.get()))
                .id().into();
        });
    });
    Host::flush(&mut patch);
    for size in [640.0, 900.0, 720.0] {
        patch.clear();
        Host::with(|h| h.set_window(Vector2::new(size, 600.0)));
        Host::flush(&mut patch);
        assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Bind { id, prop: Prop::Offset, bind: Bind::Set(_) } if *id == pane
        )), "a resize published no direct pane offset: {:#?}", patch.ops());
        assert!(!patch.ops().iter().any(|op| matches!(op, Op::Bind { bind: Bind::Animate(_), .. })));
    }
    patch.clear();
    width.set(0.0);
    Host::flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Bind { id, prop: Prop::Size, bind: Bind::Animate(Anim::Spring { tuning: windows_scene::Tuning::Layout, .. }) } if *id == pane
    )));
    drop(held);
}

#[test]
fn a_revealed_node_sets_its_bounds_before_it_springs() {
    let mut patch = fixture();
    Host::with(|h| h.set_window(Vector2::new(800.0, 600.0)));
    let (hidden, width) = (Cell::new(true), Cell::new(0.0));
    let mut mark = NodeId::NONE;
    let held = Ui::mount_root(|ui| {
        ui.node(Preset::Row).animate_layout().grow().children(|ui| {
            ui.node(Preset::Layer).layout_from(move |l| l.width = Len::dip(width.get()));
            mark = ui.node(Preset::Layer).hide_if(hidden)
                .width(Len::dip(40.0)).height(Len::dip(20.0)).id().into();
        });
    });
    Host::flush(&mut patch);
    patch.clear();
    width.set(200.0);
    hidden.set(false);
    Host::flush(&mut patch);
    for prop in [Prop::Offset, Prop::Size] {
        assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Bind { id, prop: p, bind: Bind::Set(_) } if *id == mark && *p == prop
        )), "a revealed node did not set its {prop:?}: {:#?}", patch.ops());
    }
    patch.clear();
    width.set(300.0);
    Host::flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Bind { id, prop: Prop::Offset, bind: Bind::Animate(Anim::Spring { tuning: windows_scene::Tuning::Layout, .. }) } if *id == mark
    )));
    drop(held);
}

#[test]
fn geometry_settles_without_animation_through_the_entire_creation_publication() {
    let mut patch = fixture();
    let mut parent = NodeId::NONE;
    let held = Ui::mount_root(|ui| {
        parent = ui.node(Preset::Layer).animate_layout().id().into();
    });
    let sprite = Host::with(|h| {
        let sprite = h.visual(windows_scene::GroupId(parent), None);
        h.visual_rect(sprite, Vector2::zero(), Vector2::zero());
        h.tree.touch(sprite.0);
        h.tree.encode(&mut h.pending);
        h.visual_rect(sprite, Vector2::zero(), Vector2::new(100.0, 40.0));
        sprite
    });
    Host::flush(&mut patch);
    assert!(!patch.ops().iter().any(|op| matches!(op,
        Op::Bind { id, bind: Bind::Animate(_), .. } if *id == sprite.0
    )));
    patch.clear();
    Host::with(|h| h.visual_rect(sprite, Vector2::zero(), Vector2::new(200.0, 40.0)));
    Host::flush(&mut patch);
    assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Bind { id, prop: Prop::Size, bind: Bind::Animate(Anim::Spring { tuning: windows_scene::Tuning::Layout, .. }) } if *id == sprite.0
    )));
    drop(held);
}

#[test]
fn a_patch_is_stamped_with_the_environment_it_was_solved_under() {
    let mut coarse = Rig::at(800.0, 600.0, 1.0);
    let stamped = coarse
        .mount(|ui| {
            boxed(ui, 100.0, 40.0);
        })
        .patch()
        .env;
    drop(coarse);
    let mut fine = Rig::at(800.0, 600.0, 2.0);
    let other = fine
        .mount(|ui| {
            boxed(ui, 100.0, 40.0);
        })
        .patch()
        .env;
    assert!(stamped.is_some() && other.is_some());
    assert_ne!(stamped, other, "two scales stamped the same environment");
}

// ── the builder ─────────────────────────────────────────────────────────────────────

#[test]
fn children_are_ordered_bottom_first_and_a_later_sibling_sits_above() {
    let mut rig = Rig::new();
    let (mut under, mut over) = (None, None);
    let frame = rig.mount(|ui| {
        ui.layer(|ui| {
            under = Some(button(ui, "under").name("under").id().into());
            over = Some(button(ui, "over").name("over").id().into());
        })
        .width(Len::dip(120.0))
        .height(Len::dip(40.0));
    });
    let (under, over): (NodeId, NodeId) = (under.unwrap(), over.unwrap());
    let order: Vec<ControlId> = frame.hits().iter().map(|entry| entry.id).collect();
    let (first, second) = Host::with(|h| (h.control_of(under), h.control_of(over)));
    assert!(
        order.iter().position(|&id| id == first) < order.iter().position(|&id| id == second),
        "the array is not paint order"
    );
    let (x, y) = centre(over);
    assert_eq!(
        frame.pick(x, y, ContactKind::Mouse),
        Some(over),
        "the later sibling did not sit above"
    );
}

#[test]
fn a_plate_declared_before_the_body_stays_beneath_the_bodys_first_child() {
    let mut rig = Rig::new();
    let (mut plated, mut child) = (None, None);
    rig.mount(|ui| {
        plated = Some(
            ui.node(Preset::Stack)
                .plate(Len::ZERO, Role::Fill(Fill::Surface), 1.0)
                .children(|ui| child = Some(boxed(ui, 50.0, 20.0)))
                .id()
                .into(),
        );
    });
    let (plated, child): (NodeId, NodeId) = (plated.unwrap(), child.unwrap());
    let order: Vec<NodeId> = Host::with(|h| h.tree.children(plated).collect());
    assert_eq!(order.len(), 2, "one plate and one child: {order:?}");
    assert_eq!(
        order[1], child,
        "the body's first child sat beneath the plate"
    );
}

#[test]
fn a_containers_closure_runs_with_that_container_as_the_parent() {
    let mut rig = Rig::new();
    let (mut outer, mut inner) = (None, None);
    rig.mount(|ui| {
        outer = Some(
            ui.node(Preset::Stack)
                .width(Len::dip(200.0))
                .padding(Len::dip(10.0))
                .children(|ui| inner = Some(boxed(ui, 50.0, 20.0)))
                .id()
                .into(),
        );
    });
    let (outer, inner): (NodeId, NodeId) = (outer.unwrap(), inner.unwrap());
    Host::with(|h| {
        assert_eq!(h.geom(inner).rect.x0, h.geom(outer).rect.x0 + 10.0);
        assert_eq!(h.geom(inner).local, Vector2 { x: 10.0, y: 10.0 });
    });
}

#[test]
fn editing_through_an_expired_handle_answers_absence() {
    let mut rig = Rig::new();
    let mut held = None;
    rig.mount(|ui| held = Some(ui.node(Preset::Layer).id()));
    let held = held.unwrap();
    rig.unmount();
    let mut reached = true;
    rig.mount(|ui| reached = ui.edit(held).is_some());
    assert!(!reached, "an expired handle reached a node");
}

#[test]
fn a_constant_false_branch_creates_no_content() {
    let mut rig = Rig::new();
    rig.flush();
    let before = Host::with(|h| h.live_nodes());
    rig.mount(|ui| {
        ui.when(false, |ui| {
            boxed(ui, 100.0, 40.0);
        });
    });
    assert_eq!(
        Host::with(|h| h.live_nodes()),
        before,
        "a constant false branch minted a node"
    );
}

#[test]
fn two_adjacent_branches_keep_their_order_across_being_empty() {
    let mut rig = Rig::new();
    let (first, second) = (Cell::new(true), Cell::new(true));
    rig.mount(|ui| {
        ui.when(first, |ui| {
            label(ui, "first");
        });
        ui.when(second, |ui| {
            label(ui, "second");
        });
    });
    for (a, b) in [(false, true), (true, true), (true, false), (true, true)] {
        first.set(a);
        let want: Vec<&str> = [("first", a), ("second", b)]
            .into_iter()
            .filter_map(|(name, on)| on.then_some(name))
            .collect();
        assert_eq!(
            rig.set(second, b).runs(),
            want,
            "the branches swapped order"
        );
    }
}

#[test]
fn a_keyed_list_moves_survivors_rather_than_reminting_them() {
    let mut rig = Rig::new();
    let keys = Cell::new([1u64, 2, 3]);
    rig.mount(|ui| {
        ui.each(
            move |out: &mut Vec<u64>| out.extend(keys.get()),
            |key| key,
            |ui, key| {
                text(ui, format!("row {key}"));
            },
        );
    });
    rig.flush();
    let before = Host::with(|h| h.live_nodes());
    let frame = rig.set(keys, [3, 1, 2]);
    assert_eq!(
        Host::with(|h| h.live_nodes()),
        before,
        "a survivor was reminted"
    );
    assert_eq!(frame.ops(Kind::New), 0, "a reorder minted a visual");
    assert!(frame.ops(Kind::Move) > 0, "a reorder moved nothing");
}

// ── bindings ────────────────────────────────────────────────────────────────────────

#[test]
fn a_constant_writes_the_record_and_installs_no_writer() {
    let mut rig = Rig::new();
    let mut node = None;
    let frame = rig.mount(|ui| node = Some(ui.node(Preset::Layer).opacity(0.25).id().into()));
    let node: NodeId = node.unwrap();
    assert_eq!(frame.bound(node, Prop::Opacity), Some(Value::Scalar(0.25)));
    let before = crate::signal::live_nodes();
    rig.mount(|ui| {
        ui.node(Preset::Layer).opacity(0.25);
    });
    assert_eq!(
        crate::signal::live_nodes(),
        before,
        "a constant installed a graph node"
    );
}

#[test]
fn a_reactive_source_writes_once_per_distinct_value() {
    let mut rig = Rig::new();
    let alpha = Cell::new(0.5f32);
    let mut node = None;
    rig.mount(|ui| node = Some(ui.node(Preset::Layer).opacity(alpha).id().into()));
    let node: NodeId = node.unwrap();
    let at = |v: f32| Some(Value::Scalar(v));
    assert_eq!(rig.set(alpha, 0.75).bound(node, Prop::Opacity), at(0.75));
    assert_eq!(
        rig.set(alpha, 0.75).bound(node, Prop::Opacity),
        None,
        "an unchanged value reached the wire"
    );
    assert_eq!(rig.set(alpha, 0.25).bound(node, Prop::Opacity), at(0.25));
}

#[test]
fn repeating_a_singleton_handler_replaces_it() {
    let mut rig = Rig::new();
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let (first, second) = (count.clone(), count.clone());
    let mut control = None;
    rig.mount(|ui| {
        control = Some(
            button(ui, "once")
                .on_click(move || first.set(first.get() + 1))
                .on_click(move || second.set(second.get() + 10))
                .id()
                .into(),
        );
    });
    let control: NodeId = control.unwrap();
    let target = Host::with(|h| h.control_of(control));
    Host::dispatch(&[Intent {
        target,
        what: What::Tapped,
    }]);
    assert_eq!(count.get(), 10, "the displaced handler still ran");
}

#[test]
fn a_geometry_channel_snaps_and_a_chrome_channel_springs() {
    let mut rig = Rig::new();
    let alpha = Cell::new(0.5f32);
    let mut node = None;
    rig.mount(|ui| {
        node = Some(
            ui.node(Preset::Layer)
                .width(Len::dip(40.0))
                .opacity(alpha)
                .id()
                .into(),
        );
    });
    let node: NodeId = node.unwrap();
    // The first write of any channel snaps, so the spring is what the second one carries.
    rig.set(alpha, 0.75);
    let sprung = rig.set(alpha, 0.25).patch().ops().iter().any(|op| {
        matches!(op, Op::Bind { id, prop, bind }
            if *id == node && *prop == Prop::Opacity && matches!(bind, Bind::Animate(Anim::Spring { .. })))
    });
    assert!(sprung, "a chrome channel did not spring");
    let root = Host::with(|h| h.root());
    let snapped = rig.resize(600.0, 400.0).patch().ops().iter().any(|op| {
        matches!(op, Op::Bind { id, prop, bind }
            if *id == root && *prop == Prop::Size && matches!(bind, Bind::Set(_)))
    });
    assert!(snapped, "a geometry channel did not snap");
}

// ── mount and retirement ────────────────────────────────────────────────────────────

#[test]
fn retiring_a_subtree_is_one_op_and_reclaims_every_id() {
    let mut rig = Rig::new();
    rig.flush();
    let before = Host::with(|h| h.live_nodes());
    rig.mount(|ui| {
        ui.stack(|ui| {
            for _ in 0..4 {
                boxed(ui, 40.0, 20.0);
            }
        });
    });
    assert_eq!(
        rig.unmount().ops(Kind::Drop),
        1,
        "a subtree cost more than one op"
    );
    assert_eq!(
        Host::with(|h| h.live_nodes()),
        before,
        "an id was not reclaimed"
    );
}

#[test]
fn unmounting_releases_every_row_the_subtree_claimed() {
    let mut rig = Rig::new();
    let empty = rig.flush().rows();
    let frame = rig.mount(|ui| {
        button(ui, "press").on_click(|| {}).tip("help");
        knob(ui, Cell::new(0.5), Range::UNIT).live(Cell::new(None));
    });
    assert!(frame.rows() > empty, "the mount claimed no rows");
    assert_eq!(rig.unmount().rows(), empty, "a row outlived its subtree");
}

// ── the encode ──────────────────────────────────────────────────────────────────────

#[test]
fn a_box_that_moved_is_published_and_one_that_did_not_is_not() {
    let mut rig = Rig::new();
    let width = Cell::new(100.0f32);
    let (mut sized, mut fixed) = (None, None);
    rig.mount(|ui| {
        ui.row(|ui| {
            sized = Some(
                ui.node(Preset::Layer)
                    .height(Len::dip(20.0))
                    .layout_from(move |l| l.width = Len::dip(width.get()))
                    .id()
                    .into(),
            );
            fixed = Some(boxed(ui, 30.0, 20.0));
        })
        .justify(Align::End);
    });
    let (sized, fixed): (NodeId, NodeId) = (sized.unwrap(), fixed.unwrap());
    let frame = rig.set(width, 140.0);
    let now = Host::with(|h| h.geom(sized).size);
    assert_eq!(frame.bound(sized, Prop::Size), Some(Value::Vec2(now)));
    assert_eq!(
        frame.bound(fixed, Prop::Size),
        None,
        "an unmoved box restated its size"
    );
}

/// A clip is restated from the box the solve published, so the last one on the wire is the
/// one the compositor holds.
fn last_clip(patch: &windows_scene::SinkPatch, node: NodeId) -> Option<windows_scene::Clip> {
    patch.ops().iter().rev().find_map(|op| match op {
        Op::Clip { id, clip } if *id == node => Some(*clip),
        _ => None,
    })
}

#[test]
fn a_field_clips_to_its_own_box_and_follows_it() {
    use windows_scene::Clip;
    let mut rig = Rig::new();
    let width = Cell::new(120.0f32);
    let mut field = None;
    let frame = rig.mount(|ui| {
        field = Some(
            crate::widget::field(ui, "12")
                .layout_from(move |l| l.width = Len::dip(width.get()))
                .id()
                .into(),
        );
    });
    let field: NodeId = field.unwrap();
    let size = Host::with(|h| h.geom(field).size);
    assert!(size.x > 0.0 && size.y > 0.0);
    let expected = |size: Vector2| Clip::Rect {
        l: 0.0,
        t: 0.0,
        r: size.x,
        b: size.y,
        radius: windows_scene::Corners::default(),
    };
    assert_eq!(
        last_clip(frame.patch(), field),
        Some(expected(size)),
        "a field was clipped away"
    );
    let frame = rig.set(width, 200.0);
    let size = Host::with(|h| h.geom(field).size);
    assert_eq!(size.x, 200.0);
    assert_eq!(last_clip(frame.patch(), field), Some(expected(size)));
}

#[test]
fn a_node_carried_by_its_parent_restates_no_offset_of_its_own() {
    let mut rig = Rig::new();
    let gap = Cell::new(0.0f32);
    let (mut parent, mut child) = (None, None);
    rig.mount(|ui| {
        ui.stack(|ui| {
            ui.node(Preset::Layer)
                .height(Len::dip(10.0))
                .layout_from(move |l| l.height = Len::dip(10.0 + gap.get()));
            parent = Some(
                ui.node(Preset::Stack)
                    .width(Len::dip(60.0))
                    .children(|ui| child = Some(boxed(ui, 20.0, 20.0)))
                    .id()
                    .into(),
            );
        });
    });
    let (parent, child): (NodeId, NodeId) = (parent.unwrap(), child.unwrap());
    let frame = rig.set(gap, 25.0);
    assert!(
        frame.bound(parent, Prop::Offset).is_some(),
        "the parent did not move"
    );
    assert_eq!(
        frame.bound(child, Prop::Offset),
        None,
        "a carried node restated its own offset"
    );
}

// ── the one hit array ───────────────────────────────────────────────────────────────

#[test]
fn the_array_is_paint_order_and_the_id_index_is_id_order() {
    let mut rig = Rig::new();
    let mut declared = Vec::new();
    let frame = rig.mount(|ui| {
        ui.stack(|ui| {
            for name in ["a", "b", "c"] {
                declared.push(button(ui, name).height(Len::dip(20.0)).id().into());
            }
        });
    });
    let want: Vec<ControlId> =
        Host::with(|h| declared.iter().map(|&node| h.control_of(node)).collect());
    let got: Vec<ControlId> = frame
        .hits()
        .iter()
        .map(|entry| entry.id)
        .filter(|id| want.contains(id))
        .collect();
    assert_eq!(got, want, "the array is not declaration order");
    let index = frame.patch().ops().iter().find_map(|op| match op {
        Op::Hits { index, .. } => Some(frame.patch().index(*index)),
        _ => None,
    });
    let index = index.expect("the publication carried no id index");
    assert!(
        index.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "the id index is unsorted"
    );
}

#[test]
fn touch_inflation_never_lets_two_targets_claim_one_point() {
    let mut rig = Rig::new();
    let (mut outer, mut inner) = (None, None);
    let frame = rig.mount(|ui| {
        outer = Some(
            ui.control(None, UiaRole::Button, |ui| {
                inner = Some(
                    button(ui, "x")
                        .width(Len::dip(12.0))
                        .height(Len::dip(12.0))
                        .no_inflate()
                        .id()
                        .into(),
                );
            })
            .width(Len::dip(200.0))
            .height(Len::dip(48.0))
            .id()
            .into(),
        );
    });
    let (outer, inner): (NodeId, NodeId) = (outer.unwrap(), inner.unwrap());
    let (x, y) = centre(inner);
    assert_eq!(
        frame.pick(x, y, ContactKind::Touch),
        Some(inner),
        "the inner target lost its own box"
    );
    assert_eq!(
        frame.pick(180.0, y, ContactKind::Touch),
        Some(outer),
        "the outer target lost the rest"
    );
}

#[test]
fn a_scroll_rail_declares_a_control_and_no_handler_row() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        scroll(ui, |ui| {
            for _ in 0..40 {
                boxed(ui, 100.0, 30.0);
            }
        })
        .height(Len::times(Metric::RowH, 6.0));
    });
    Host::with(|h| {
        assert!(
            h.controls.iter().count() > 0,
            "the rail declared no control"
        );
        assert_eq!(h.handlers.placed(), 0, "the rail placed a handler row");
    });
}

// ── the automation rows the same walk emits ─────────────────────────────────────────

#[test]
fn a_control_is_named_by_the_text_its_subtree_laid_out() {
    let mut rig = Rig::new();
    let mut frame = rig.mount(|ui| {
        button(ui, "Apply");
    });
    let row = frame.uia("Apply").expect("the button published no element");
    assert_eq!(row.role, UiaRole::Button);
    assert!(
        row.flags.has(ColFlags::FOCUSABLE),
        "a button is not focusable"
    );
}

#[test]
fn an_element_with_no_role_is_skipped_and_its_children_reparent_past_it() {
    let mut rig = Rig::new();
    let mut frame = rig.mount(|ui| {
        ui.control(None, UiaRole::Group, |ui| {
            ui.node(Preset::Stack).children(|ui| {
                button(ui, "inner").name("inner");
            });
        })
        .name("outer");
    });
    let outer = frame.uia("outer").expect("the group published no element");
    assert_eq!(
        outer.children,
        ["inner"],
        "a bare container took a row of its own"
    );
}

#[test]
fn a_disabled_control_keeps_its_entry_and_states_that_it_is_disabled() {
    let mut rig = Rig::new();
    let off = Cell::new(false);
    rig.mount(|ui| {
        button(ui, "Apply").name("Apply").disabled(off);
    });
    let row = rig
        .set(off, true)
        .uia("Apply")
        .expect("a disabled control lost its element");
    assert!(
        !row.state.has(State::ENABLED),
        "a disabled control reported enabled"
    );
}

#[test]
fn selection_and_disabled_bindings_preserve_each_other() {
    for disabled_first in [false, true] {
        let mut rig = Rig::new();
        let selected = Cell::new(true);
        let disabled = Cell::new(false);
        let mut frame = rig.mount(|ui| {
            let item = button(ui, "Channel")
                .name("Channel")
                .role(UiaRole::CheckBox);
            if disabled_first {
                item.disabled(disabled).selected(selected);
            } else {
                item.selected(selected).disabled(disabled);
            }
        });
        let row = frame.uia("Channel").unwrap();
        assert!(row.state.has(State::ENABLED));
        assert!(row.state.has(State::TOGGLED));
        let row = rig.set(disabled, true).uia("Channel").unwrap();
        assert!(!row.state.has(State::ENABLED));
        assert!(row.state.has(State::TOGGLED));
        let row = rig.set(selected, false).uia("Channel").unwrap();
        assert!(!row.state.has(State::ENABLED));
        assert!(!row.state.has(State::TOGGLED));
        let row = rig.set(selected, true).uia("Channel").unwrap();
        assert!(!row.state.has(State::ENABLED));
        assert!(row.state.has(State::TOGGLED));
        let row = rig.set(disabled, false).uia("Channel").unwrap();
        assert!(row.state.has(State::ENABLED));
        assert!(row.state.has(State::TOGGLED));
    }
}

#[test]
fn a_slider_publishes_the_range_it_runs_over() {
    let mut rig = Rig::new();
    let mut frame = rig.mount(|ui| {
        knob(ui, Cell::new(0.5), Range::new(0.0, 12.0).step(0.1)).name("Gain");
    });
    let row = frame.uia("Gain").expect("the knob published no element");
    assert!(row.flags.has(ColFlags::RANGED), "a slider is not ranged");
    let range = row.range.expect("a slider published no bounds");
    assert_eq!((range.min, range.max), (0.0, 12.0));
    assert_eq!(range.step, 0.1);
}

#[test]
fn a_disclosure_dispatches_the_requested_state() {
    let mut rig = Rig::new();
    let expanded = Cell::new(false);
    let mut target = ControlId::NONE;
    rig.mount(|ui| {
        target = button(ui, "Details")
            .expanded(expanded)
            .on_expand(move |open| expanded.set(open))
            .control_id();
    });
    for open in [false, true, true, false, false] {
        Host::dispatch(&[Intent {
            target,
            what: What::Expanded(open),
        }]);
        let row = rig.set(expanded, open).uia("Details").unwrap();
        assert!(row.flags.has(ColFlags::EXPANDS));
        assert_eq!(row.state.has(State::EXPANDED), open);
    }
}

#[test]
fn numeric_binding_preserves_drag_and_publishes_dynamic_bounds() {
    let mut rig = Rig::new();
    let bounds = Cell::new(12.0);
    let mut target = ControlId::NONE;
    rig.mount(|ui| {
        target = ui
            .node(Preset::Layer)
            .name("Band")
            .on_drag(crate::gesture::DragDecl::default(), |_| {})
            .range_value(
                move || Range::new(-bounds.get(), bounds.get()).step(0.1),
                crate::widget::ScalarValue {
                    value: 2.0,
                    epoch: 0,
                },
            )
            .on_gesture(|_| {})
            .control_id();
    });
    let row = rig.set(bounds, 24.0).uia("Band").unwrap();
    assert_eq!(row.range.unwrap(), Range::new(-24.0, 24.0).step(0.1));
    assert!(!row.flags.has(ColFlags::READ_ONLY));
    Host::with(|host| {
        let row = host.control(target).unwrap();
        assert!(row.front.flags & crate::widget::flag::DRAGS != 0);
        assert!(host.handlers.get(row.handlers).unwrap().drag.is_some());
    });
}

#[test]
fn choice_navigation_crosses_layout_wrappers_and_skips_disabled_items() {
    let mut rig = Rig::new();
    let mut ids = [ControlId::NONE; 3];
    rig.mount(|ui| {
        ui.node(Preset::Stack)
            .role(UiaRole::Tab)
            .selection(true)
            .children(|ui| {
                for (at, id) in ids.iter_mut().enumerate() {
                    ui.node(Preset::Row).children(|ui| {
                        *id = button(ui, "Page")
                            .role(UiaRole::TabItem)
                            .selected(at == 0)
                            .disabled(at == 1)
                            .control_id();
                    });
                }
            });
    });
    Host::with(|h| {
        assert_eq!(h.choice_neighbor(ids[0], 0x27), Some(ids[2]));
        assert_eq!(h.choice_neighbor(ids[2], 0x27), Some(ids[0]));
        assert_eq!(h.choice_neighbor(ids[0], 0x25), Some(ids[2]));
        assert_eq!(h.choice_neighbor(ids[2], 0x24), Some(ids[0]));
    });
}

#[test]
fn explicit_tab_policy_overrides_selection_and_sends_only_changes() {
    let mut rig = Rig::new();
    let selected = Cell::new(true);
    let value = Cell::new(1);
    let mut id = ControlId::NONE;
    rig.mount(|ui| {
        id = button(ui, "Preset")
            .role(UiaRole::RadioButton)
            .selected(selected)
            .tab_stop(move || value.get() >= 0)
            .control_id();
    });
    Host::with(|h| h.focus_ops.clear());
    rig.set(selected, false);
    rig.set(value, 2);
    Host::with(|h| assert!(h.focus_ops.is_empty()));
    rig.set(value, -1);
    Host::with(|h| assert_eq!(h.focus_ops, [crate::seam::FocusOp::TabIndex(id, -1)]));
}

#[test]
fn stock_slider_centres_its_thumb_on_the_rail_in_both_orientations() {
    use crate::widget::{ScalarPart, SliderStyle, slider};
    for scale in [1.0, 1.5, 2.0] {
        for vertical in [false, true] {
            let mut rig = Rig::at(800.0, 600.0, scale);
            let mut node = NodeId::NONE;
            rig.mount(|ui| {
                let range = Range {
                    vertical,
                    ..Range::new(-12.0, 12.0)
                };
                node = slider(ui, 0.0, range, SliderStyle::default())
                    .width(Len::dip(if vertical { 30.0 } else { 300.0 }))
                    .height(Len::dip(if vertical { 300.0 } else { 30.0 }))
                    .id()
                    .into();
            });
            Host::with(|host| {
                let row = host.control(host.control_of(node)).unwrap();
                let value = row.value.unwrap();
                let thumb = value
                    .parts
                    .iter()
                    .find_map(|&(node, part)| {
                        matches!(part, ScalarPart::Thumb { .. }).then_some(host.geom(node))
                    })
                    .unwrap();
                let own = host.geom(node);
                let centre = if vertical {
                    thumb.local.x + thumb.size.x * 0.5
                } else {
                    thumb.local.y + thumb.size.y * 0.5
                };
                let expected = if vertical { own.size.x } else { own.size.y } * 0.5;
                assert!(
                    (centre - expected).abs() < 0.51 / scale,
                    "vertical={vertical}, scale={scale}: thumb {centre}, rail {expected}"
                );
            });
            assert!(rig.flush().patch().ops().is_empty());
        }
    }
}

#[test]
fn a_live_region_states_how_it_announces() {
    let mut rig = Rig::new();
    let mut frame = rig.mount(|ui| {
        ui.control(None, UiaRole::Text, |ui| {
            text(ui, "3 items");
        })
        .name("status")
        .live_region(false);
    });
    let row = frame
        .uia("status")
        .expect("the region published no element");
    assert!(
        row.flags.has(ColFlags::LIVE_POLITE),
        "a live region did not state how it announces"
    );
}

/// A group announced as one run is named by the run inside it.
#[test]
fn a_text_group_is_named_by_its_run() {
    let mut rig = Rig::new();
    let mut frame = rig.mount(|ui| {
        crate::widget::text_group(ui).children(|ui| {
            text(ui, "nothing here");
        });
    });
    let row = frame
        .uia("nothing here")
        .expect("the group published no element");
    assert_eq!(row.role, UiaRole::Text);
}

#[test]
fn a_selected_control_states_that_it_selects() {
    let mut rig = Rig::new();
    let on = Cell::new(false);
    rig.mount(|ui| {
        button(ui, "Tab").name("Tab").selected(on);
    });
    let row = rig
        .set(on, true)
        .uia("Tab")
        .expect("the control lost its element");
    assert!(
        row.state.has(State::SELECTED),
        "a selected control did not report selected"
    );
    assert!(
        row.flags.has(ColFlags::SELECTS),
        "a selectable control does not answer SelectionItem"
    );
}

/// The lane's edge tab: a grid row hidden while the pane is docked, holding a control with
/// a glyph and a rotated label.
fn tab_fixture(ui: &mut Ui<'_>, shown: Cell<bool>) -> (NodeId, NodeId) {
    use crate::layout::{Edge, Track};
    use crate::role::TypeRole;
    use crate::widget::{TextStyle, edge_button, styled_text};
    let (mut tab, mut lbl) = (None, None);
    ui.node(Preset::Grid)
        .rows([Track::fr(1.0), Track::fr(1.0)])
        .cols([Track::fr(1.0)])
        .children(|ui| {
            ui.node(Preset::Row).at(1, 0).children(|ui| {
                boxed(ui, 300.0, 100.0);
            });
            ui.node(Preset::Row)
                .children(|ui| {
                    ui.node(Preset::Layer).grow();
                    ui.node(Preset::Stack).children(|ui| {
                        ui.node(Preset::Layer).grow();
                        tab = Some(
                            edge_button(ui, "", Edge::Right, Metric::Radius)
                                .name("Show")
                                .stack(|ui| {
                                    label(ui, "‹");
                                    let vertical = TextStyle::new(TypeRole::Body).vertical(true);
                                    lbl = Some(styled_text(ui, "Inspector", vertical).id().into());
                                })
                                .id()
                                .into(),
                        );
                    });
                })
                .span(0, 0, 2, 1)
                .align(Align::Stretch)
                .hide_if(shown);
        });
    (tab.unwrap(), lbl.unwrap())
}

#[test]
fn a_node_given_no_setter_is_measured_on_its_first_solve() {
    let mut rig = Rig::at(1900.0, 1000.0, 1.5);
    let mut ids = None;
    rig.mount(|ui| ids = Some(tab_fixture(ui, Cell::new(false))));
    let (tab, lbl) = ids.unwrap();
    let (tab, lbl) = Host::with(|h| (h.geom(tab).rect, h.geom(lbl).rect));
    assert!(
        tab.width() > 30.0 && tab.height() > 60.0,
        "the tab is its padding alone: {tab:?}"
    );
    assert!(
        lbl.height() > lbl.width() && lbl.width() > 0.0,
        "the label was not measured: {lbl:?}"
    );
}

#[test]
fn a_subtree_hidden_at_mount_takes_its_size_when_shown() {
    let mut rig = Rig::at(1900.0, 1000.0, 1.5);
    let shown = Cell::new(true);
    let mut ids = None;
    rig.mount(|ui| ids = Some(tab_fixture(ui, shown)));
    let (tab, lbl) = ids.unwrap();
    let mut visible = None;
    rig.mount(|ui| visible = Some(tab_fixture(ui, Cell::new(false))));
    let expect = Host::with(|h| h.geom(visible.unwrap().0).rect);
    rig.set(shown, false);
    let (got, lbl) = Host::with(|h| (h.geom(tab).rect, h.geom(lbl).rect));
    // The second root is mounted beneath the first, so the origin differs and the box is
    // what has to agree.
    assert_eq!(
        (got.width(), got.height(), got.x1),
        (expect.width(), expect.height(), expect.x1),
        "shown from a hidden mount: {got:?} vs {expect:?}"
    );
    assert!(
        lbl.width() > 0.0,
        "the label under it stayed unmeasured: {lbl:?}"
    );
}

#[test]
fn a_hidden_subtrees_derived_sprites_take_no_pixels() {
    let mut rig = Rig::at(1900.0, 1000.0, 1.5);
    let shown = Cell::new(false);
    let mut ids = None;
    rig.mount(|ui| ids = Some(tab_fixture(ui, shown)));
    let (_, lbl) = ids.unwrap();
    // The line tile under the text leaf carries its own rect, so it is the one to watch.
    let tile = Host::with(|h| h.tree.children(lbl).next()).expect("a line tile");
    let size = |n: NodeId| Host::with(|h| h.geom(n).size);
    assert!(
        size(tile).x > 0.0 && size(tile).y > 0.0,
        "the tile has no ink: {:?}",
        size(tile)
    );
    rig.set(shown, true);
    assert_eq!(
        size(tile),
        Vector2::zero(),
        "a hidden ancestor left the tile painting"
    );
    rig.set(shown, false);
    assert!(
        size(tile).x > 0.0,
        "showing again did not restore the tile: {:?}",
        size(tile)
    );
}

/// A run that wraps to more lines takes more room, and what follows it moves down.
///
/// The box a run is given and the lines it paints are one statement: a container that kept the
/// height the run measured before its string changed draws the next child over the run's own
/// second line, and nothing about the result reads as a layout fault.
#[test]
fn a_run_that_rewraps_moves_what_follows_it() {
    let mut rig = Rig::at(800.0, 600.0, 1.0);
    let long = Cell::new(false);
    let mut ids = None;
    rig.mount(|ui| {
        let mut run = NodeId::NONE;
        let mut after = NodeId::NONE;
        box_(ui).width(Len::dip(120.0)).stack(|ui| {
            run = caption(
                ui,
                crate::widget::reactive(move |out| {
                    out.push_str(if long.get() {
                        "Computing the response."
                    } else {
                        "Idle."
                    });
                }),
            )
            .node_id();
            after = micro(ui, "9 sections").node_id();
        });
        ids = Some((run, after));
    });
    let (run, after) = ids.expect("mounted");
    let box_of = |n: NodeId| Host::with(|h| h.geom(n).rect);
    let one = box_of(run).height();
    let below = box_of(after).y0;

    rig.set(long, true);
    let two = box_of(run).height();
    assert!(
        two > one,
        "a run that wrapped to more lines kept its old height: {one} then {two}"
    );
    assert!(
        box_of(after).y0 >= below + (two - one),
        "the run grew and what follows it stayed put: {} then {}",
        below,
        box_of(after).y0
    );
}

/// A run's published box holds the width its height was solved against.
///
/// The far edge is snapped onto the pixel grid, and at a fractional scale rounding to nearest
/// can land a fraction of a DIP inside the extent the run was measured to need. The run then
/// breaks a second line inside a box one line tall and draws over whatever follows it, which
/// reads as anything but a rounding fault. 1.5 is where it shows: at 1.0 every DIP is already
/// a pixel.
#[test]
fn a_content_sized_run_is_not_snapped_below_its_own_width() {
    let mut rig = Rig::at(1000.0, 400.0, 1.5);
    let mut ids = None;
    rig.mount(|ui| {
        let mut run = NodeId::NONE;
        let mut after = NodeId::NONE;
        box_(ui)
            .gap(Metric::SpaceXs)
            .padding(Metric::SpaceSm)
            .justify(Align::Center)
            .grow()
            .children(|ui| {
                run = caption(ui, "Computing the response.").node_id();
                after = micro(ui, "9 sections").node_id();
            });
        ids = Some((run, after));
    });
    let (run, after) = ids.expect("mounted");
    Host::with(|h| {
        let key = h.tree.c.text[run.index()];
        let class = h.tree.class(run);
        let widest = h.text.pair(key, class)[1];
        let box_ = h.geom(run).rect;
        assert!(
            box_.width() >= widest,
            "the box was snapped inside the run's own width: {} for {widest}",
            box_.width()
        );
        // One tile per line, so the count is what the run actually broke into.
        assert_eq!(
            h.tree.children(run).count(),
            1,
            "the run broke a line the box has no room for"
        );
        assert!(
            h.geom(after).rect.y0 >= box_.y1,
            "what follows the run starts inside it: {:?} under {box_:?}",
            h.geom(after).rect
        );
    });
}

#[test]
fn optional_group_dispatches_deselection_and_rechecks_queued_additions() {
    use crate::uia::action::SelectionChange::{Add, Remove, Select};
    let mut rig = Rig::new();
    let chosen = Cell::new(Some(0usize));
    let mut ids = [ControlId::NONE; 2];
    rig.mount(|ui| {
        ui.node(Preset::Stack).selection(false).children(|ui| {
            for (at, id) in ids.iter_mut().enumerate() {
                *id = button(ui, format!("Choice {at}"))
                    .role(UiaRole::RadioButton)
                    .selected(move || chosen.get() == Some(at))
                    .on_select(move |selected| chosen.set(selected.then_some(at)))
                    .control_id();
            }
        });
    });
    Host::dispatch(&[
        Intent {
            target: ids[0],
            what: What::Selected(Remove),
        },
        Intent {
            target: ids[1],
            what: What::Selected(Add),
        },
        Intent {
            target: ids[0],
            what: What::Selected(Add),
        },
    ]);
    assert_eq!(chosen.get(), Some(1));
    Host::dispatch(&[Intent {
        target: ids[0],
        what: What::Selected(Select),
    }]);
    assert_eq!(chosen.get(), Some(0));
    Host::dispatch(&[Intent {
        target: ids[0],
        what: What::Selected(Remove),
    }]);
    assert_eq!(chosen.get(), None);
}

#[test]
fn static_text_reuses_its_pinned_geometry_between_accessibility_publications() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        text(ui, "Shared geometry");
    });
    let mut first = crate::uia::Snapshot::default();
    let mut second = crate::uia::Snapshot::default();
    Host::with(|host| {
        host.uia_entries(&mut first);
        host.uia_entries(&mut second);
    });
    assert_eq!(first.text_geometry.len(), 1);
    assert!(!first.text_geometry[0].1.clusters.is_empty());
    assert!(std::sync::Arc::ptr_eq(
        &first.text_geometry[0].1,
        &second.text_geometry[0].1
    ));
}

#[test]
#[should_panic(expected = "optional selection items require on_select(bool)")]
fn optional_group_rejects_click_only_items_through_layout_wrappers() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        ui.node(Preset::Stack).selection(false).children(|ui| {
            ui.node(Preset::Row).children(|ui| {
                button(ui, "Choice").selected(true).on_click(|| {});
            });
        });
    });
}

#[test]
fn nested_required_group_keeps_its_click_handler_contract() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        ui.node(Preset::Stack).selection(false).children(|ui| {
            ui.node(Preset::Row).selection(true).children(|ui| {
                button(ui, "Choice").selected(true).on_click(|| {});
            });
        });
    });
}

#[test]
fn editable_fields_publish_text_bodies_but_passwords_do_not() {
    let mut rig = Rig::new();
    let mut ids = [ControlId::NONE; 2];
    rig.mount(|ui| {
        ids[0] = crate::widget::field(ui, "Text").control_id();
        ids[1] = crate::widget::field(ui, "Secret")
            .scope(crate::text_input::InputScope::Password)
            .control_id();
    });
    let mut snapshot = crate::uia::Snapshot::default();
    Host::with(|host| host.uia_entries(&mut snapshot));
    for (id, body) in ids.into_iter().zip([true, false]) {
        let entry = snapshot.entries.iter().find(|e| e.id == id).unwrap();
        assert!(entry.flags.has(ColFlags::FIELD));
        assert_eq!(entry.flags.has(ColFlags::BODY), body);
    }
}

#[test]
fn text_range_reveal_moves_the_field_view_without_moving_its_selection() {
    let mut rig = Rig::new();
    let mut target = ControlId::NONE;
    rig.mount(|ui| {
        target = crate::widget::field(ui, "abcdefghijklmnopqrstuvwxyz")
            .width(Len::dip(80.0))
            .control_id();
    });
    Host::with(|host| {
        host.field_update(&crate::text_input::Update {
            id: target,
            revision: 1,
            text: Some(
                "abcdefghijklmnopqrstuvwxyz"
                    .encode_utf16()
                    .collect::<Vec<_>>()
                    .into(),
            ),
            selection: crate::text_input::Selection::at(0),
            composition: None,
            focused: true,
            commit: None,
        })
    });
    rig.flush();
    let (revision, selection, before) = Host::with(|host| {
        let row = host.fields.get(target).unwrap();
        (row.revision, row.selection, row.geometry.clone().unwrap())
    });
    Host::dispatch(&[Intent {
        target,
        what: What::TextReveal {
            revision,
            start: 25,
            end: 25,
        },
    }]);
    rig.flush();
    Host::with(|host| {
        let row = host.fields.get(target).unwrap();
        let after = row.geometry.as_ref().unwrap();
        assert!(after.origin.x < before.origin.x);
        let caret = after.caret(crate::text_input::Selection::at(25));
        let x = after.origin.x + caret.x;
        assert!(x >= after.viewport.x && x <= after.viewport.x + after.viewport.w);
        assert_eq!(row.selection, selection);
        assert!(std::sync::Arc::ptr_eq(&after.clusters, &before.clusters));
    });
    let held = Host::with(|host| host.fields.get(target).unwrap().geometry.clone().unwrap());
    Host::dispatch(&[Intent {
        target,
        what: What::TextReveal {
            revision: revision + 1,
            start: 0,
            end: 0,
        },
    }]);
    rig.flush();
    Host::with(|host| {
        assert!(std::sync::Arc::ptr_eq(
            host.fields.get(target).unwrap().geometry.as_ref().unwrap(),
            &held
        ))
    });
}

#[test]
fn conditional_slide_releases_input_and_retires_during_entry() {
    let mut patch = fixture();
    let shown = Cell::new(false);
    let node = Cell::new(NodeId::NONE);
    let _mount = Ui::mount_root(|ui| {
        ui.when_slide(shown, crate::overlay::Slide {
            by: Vector2 { x: 1.0, y: 0.0 },
            ms: 200,
            easing: windows_scene::Easing::Linear,
        }, move |ui| node.set(boxed(ui, 360.0, 240.0)));
    });
    Host::flush(&mut patch);
    for complete in [true, false, true] {
        patch.clear();
        shown.set(true);
        crate::signal::flush();
        Host::flush(&mut patch);
        let id = node.get();
        assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Bind { id: target, prop: Prop::AnchorX, bind: Bind::Animate(Anim::Frames { .. }) }
            if *target == id)));
        assert!(Host::with(|h| h.input_suspended(id)));
        for width in [800.0, 790.0, 780.0] {
            patch.clear();
            Host::with(|h| h.set_window(Vector2 { x: width, y: 600.0 }));
            Host::flush(&mut patch);
            assert!(Host::with(|h| h.input_suspended(id)));
            assert!(!patch.ops().iter().any(|op| matches!(op,
                Op::Bind { id: target, prop: Prop::AnchorX | Prop::AnchorY, .. } if *target == id)),
                "resize restarted or canceled the entrance");
        }
        patch.clear();
        Host::flush(&mut patch);
        assert!(patch.ops().is_empty());
        if complete {
            Host::with(|h| {
                h.uia_published();
                h.complete_overlay_entry(id);
                assert!(h.uia_stale());
            });
            assert!(!Host::with(|h| h.input_suspended(id)));
        }
        shown.set(false);
        crate::signal::flush();
        Host::flush(&mut patch);
        assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Drop { id: target, exit: windows_scene::Exit::Slide { .. }, .. } if *target == id)));
        assert!(!Host::with(|h| h.tree.is_live(id)));
        Host::with(|h| h.complete_overlay_entry(id));
    }
}
