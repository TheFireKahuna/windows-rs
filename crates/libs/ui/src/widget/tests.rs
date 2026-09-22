//! What a stock recipe declares, asked of the host it declared into.
//!
//! The front thread's half of the value path — the writer, the wash and the reveal fade —
//! is driven against a real compositor in `scalar_tests` and `reveal_tests` beside this file.
//!
//! The claims, and where each comes from.
//!
//!  1. A toggle's knob has extent inside its track, and a flip publishes its fraction. [17-WIDGETS §8.1]
//!  2. A meter reports a value and takes no gesture. [09-AUTHORING §6]
//!  3. A part the control drives cannot also be bound by its author. [17-WIDGETS §8.2]
//!  4. At most four parts belong to the nearest control. [17-WIDGETS §8.2]
//!  5. A reveal requires an interaction scope. [17-WIDGETS §8.5]
//!  6. A control publishes its value into the application's cell. [17-WIDGETS §8.2]
//!  7. Branch churn releases a control's accessible text. [09-AUTHORING §6]
//!  8. An icon button is its side square and centres its mark, an oversize one included. [09-AUTHORING §4]

use crate::build::Host;
use crate::build::rig::Rig;
use crate::build::tree::DERIVED;
use crate::layout::{Len, Preset, Track};
use crate::signal::Cell;
use crate::widget::{Range, ScalarPart, UiaRole, icon_button, knob, meter, text, toggle};
use windows_scene::{ContactKind, NodeId};

/// Where `node`'s control stands in its own range, as the front thread is told.
fn fraction(node: NodeId) -> Option<f32> {
    Host::with(|h| {
        let id = h.control_of(node);
        h.control(id)?.value.map(|value| value.fraction)
    })
}

#[test]
fn chrome_publishes_fractional_washes_and_can_leave_highlighting_to_a_reveal() {
    use crate::role::{Fill, Role, resolve};
    use crate::widget::{Wash, button, card};

    let mut rig = Rig::new();
    let mut controls = Vec::new();
    let mut scoped = None;
    rig.mount(|ui| {
        controls.push(button(ui, "Button").id().into());
        controls.push(button(ui, "Ghost").ghost().id().into());
        controls.push(toggle(ui, Cell::new(false)).id().into());
        scoped = Some(card(ui).interaction_scope().wash(Wash::None).id().into());
    });
    Host::with(|h| {
        for node in controls {
            let row = h.control(h.control_of(node)).unwrap();
            assert!(!row.front.wash.0.is_none());
            assert_eq!(row.front.hover, resolve(Role::Fill(Fill::Hover), row.scope).a);
            assert_eq!(row.front.press, resolve(Role::Fill(Fill::Pressed), row.scope).a);
            assert!(row.front.press > 0.0 && row.front.press < row.front.hover);
            assert!(row.front.hover < 0.03);
        }
        let row = h.control(h.control_of(scoped.unwrap())).unwrap();
        assert!(row.front.wash.0.is_none());
    });
    let frame = rig.flush();
    assert!(frame.patch().ops().is_empty());
}

#[test]
fn a_toggles_knob_has_extent_inside_its_track_and_a_flip_publishes_its_fraction() {
    let mut rig = Rig::new();
    let on = Cell::new(false);
    let mut track = None;
    let frame = rig.mount(|ui| track = Some(toggle(ui, on).id().into()));
    let track: NodeId = track.unwrap();
    // The sprites the chrome derives are not the knob: the knob is what the recipe declared.
    let knob_ = Host::with(|h| {
        h.tree
            .children(track)
            .find(|child| h.tree.c.flags[child.index()] & DERIVED == 0)
            .expect("the toggle mounted no knob")
    });
    let (outer, inner) = Host::with(|h| (h.geom(track).rect, h.geom(knob_).rect));
    assert!(inner.width() > 0.0 && inner.height() > 0.0, "the knob has no extent");
    assert!(inner.x0 >= outer.x0 && inner.x1 <= outer.x1, "the knob left its track");
    assert_eq!(fraction(track), Some(0.0), "an off toggle published a fraction");
    drop(frame);
    rig.set(on, true);
    assert_eq!(fraction(track), Some(1.0), "a flip published nothing");
}

#[test]
fn a_meter_reports_a_value_and_takes_no_gesture() {
    let mut rig = Rig::new();
    let mut node = None;
    let mut frame = rig.mount(|ui| {
        node = Some(meter(ui, 0.5f32).name("Level").id().into());
    });
    let node: NodeId = node.unwrap();
    let box_ = Host::with(|h| h.geom(node).rect);
    let (x, y) = ((box_.x0 + box_.x1) * 0.5, (box_.y0 + box_.y1) * 0.5);
    assert_eq!(frame.pick(x, y, ContactKind::Mouse), None, "a meter claimed a contact");
    let row = frame.uia("Level").expect("a meter published no element");
    assert_eq!(row.role, UiaRole::ProgressBar);
}

#[test]
#[should_panic = "a scalar part cannot also bind its driven property"]
fn a_part_the_control_drives_cannot_also_be_bound_by_its_author() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        knob(ui, Cell::new(0.5), Range::UNIT).children(|ui| {
            ui.node(Preset::Layer)
                .rotation(0.25)
                .scalar_part(ScalarPart::Rotation { from: 0.0, to: 1.0 });
        });
    });
}

#[test]
#[should_panic = "a scalar supports at most four parts"]
fn at_most_four_parts_belong_to_the_nearest_control() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        knob(ui, Cell::new(0.5), Range::UNIT).children(|ui| {
            for _ in 0..5 {
                ui.node(Preset::Layer).scalar_part(ScalarPart::Thumb { vertical: false });
            }
        });
    });
}

#[test]
#[should_panic = "a reveal requires an interaction scope"]
fn a_reveal_requires_an_interaction_scope() {
    let mut rig = Rig::new();
    rig.mount(|ui| {
        ui.node(Preset::Layer).reveal_on_interaction();
    });
}

#[test]
fn a_control_publishes_its_value_into_the_applications_cell() {
    let mut rig = Rig::new();
    let (gain, live) = (Cell::new(0.25f64), Cell::new(None));
    rig.mount(|ui| {
        knob(ui, gain, Range::new(0.0, 4.0)).live(live);
    });
    assert_eq!(live.get(), Some(0.25), "the cell did not take the declared value");
    rig.set(gain, 2.0);
    assert_eq!(live.get(), Some(2.0), "the cell did not follow the source");
}

#[test]
fn a_slider_keeps_its_full_rail_separate_from_the_trimmed_trail() {
    use windows_scene::{Mask, Op};

    let mut rig = Rig::new();
    let value = Cell::new(0.0);
    let frame = rig.mount(|ui| {
        super::slider_source(
            ui,
            move || super::ScalarValue { value: value.get(), epoch: 0 },
            Range::new(-6.0, 6.0),
            super::SliderStyle { origin: Some(0.0), mark_origin: false, ..Default::default() },
        );
    });
    let paths: Vec<_> = frame.patch().ops().iter().filter_map(|op| match op {
        Op::Mask { mask: Mask::Shape { geom, stroke: Some(_) }, .. } => Some(*geom),
        _ => None,
    }).collect();
    assert_eq!(paths.len(), 2);
    assert_ne!(paths[0], paths[1], "trimming the trail must not trim the full rail");
}

#[test]
fn branch_churn_releases_a_controls_accessible_text() {
    let mut rig = Rig::new();
    let shown = Cell::new(true);
    rig.mount(|ui| {
        ui.when(shown, |ui| {
            text(ui, "Gain");
        });
    });
    assert_eq!(rig.flush().runs(), ["Gain"]);
    assert!(rig.set(shown, false).runs().is_empty(), "a retired run kept its text");
    assert_eq!(rig.set(shown, true).runs(), ["Gain"], "the branch did not come back");
}


#[test]
fn an_icon_button_is_its_side_square_and_centres_its_mark() {
    // A mark inside the box and one past it: the button states no inset, so both sit on
    // the box's centre rather than on an inset's.
    for mark in [10.0f32, 24.0] {
        let mut rig = Rig::new();
        let (mut button, mut figure) = (None, None);
        let _frame = rig.mount(|ui| {
            let element = icon_button(ui, Len::dip(16.0), |ui| {
                figure = Some(ui.node(Preset::Layer).size(Len::dip(mark)).id().into());
            });
            button = Some(element.id().into());
        });
        let (button, figure): (NodeId, NodeId) = (button.unwrap(), figure.unwrap());
        let (outer, inner) = Host::with(|h| (h.geom(button).rect, h.geom(figure).rect));
        assert_eq!((outer.width(), outer.height()), (16.0, 16.0), "mark {mark}: not its side square");
        let inset = (16.0 - mark) / 2.0;
        assert_eq!(
            (inner.x0 - outer.x0, inner.y0 - outer.y0),
            (inset, inset),
            "mark {mark}: not on the box's centre"
        );
    }
}

#[test]
fn a_run_in_a_grid_cell_is_as_wide_as_its_ink() {
    let mut rig = Rig::new();
    let (mut loose, mut celled) = (None, None);
    let _frame = rig.mount(|ui| {
        loose = Some(text(ui, "none").id().into());
        ui.node(Preset::Grid)
            .width(Len::dip(400.0))
            .cols([Track::AUTO, Track::fr(1.0)])
            .children(|ui| {
                text(ui, "Routing points");
                celled = Some(text(ui, "none").id().into());
            });
    });
    let (loose, celled): (NodeId, NodeId) = (loose.unwrap(), celled.unwrap());
    let (ink, cell) = Host::with(|h| (h.geom(loose).rect.width(), h.geom(celled).rect.width()));
    assert!(ink > 0.0, "the run measured nothing");
    assert!(
        (cell - ink).abs() <= 0.5,
        "the weighted column stretched the run: {cell} against an ink of {ink}"
    );
}

#[test]
fn a_flipped_toggle_states_a_new_revision() {
    let mut rig = Rig::new();
    let on = Cell::new(false);
    let mut track = None;
    let frame = rig.mount(|ui| track = Some(toggle(ui, on).id().into()));
    let track: NodeId = track.unwrap();
    let published = || {
        Host::with(|h| {
            let id = h.control_of(track);
            h.control(id)?.value.map(|v| (v.fraction, v.revision))
        })
    };
    let off = published().expect("a toggle publishes a value");
    drop(frame);
    rig.set(on, true);
    let lit = published().expect("a flip publishes a value");
    assert_ne!(off.0, lit.0, "the fraction did not move");
    assert_ne!(
        off.1, lit.1,
        "a repeated revision is a geometry-only update: the front keeps its own fraction \
         and the knob stays where it was"
    );
}

/// A scroll container's thumb and content ride a tracker expression on `Offset.Y`. Setting
/// `Offset` whole replaces it, so the layout must never write the composite on a node whose
/// sub-channel a binding drives.
#[test]
fn the_layout_never_writes_the_composite_offset_over_a_driven_axis() {
    use crate::layout::{Len, scroll};
    use std::collections::HashSet;
    use windows_scene::{Bind, Id, Op, Prop};
    let mut rig = Rig::new();
    let mut driven: HashSet<Id<{ windows_scene::NODE }>> = HashSet::new();
    let mut clobbered = Vec::new();
    let inspect = |frame: &crate::build::rig::Frame<'_>,
                       driven: &mut HashSet<Id<{ windows_scene::NODE }>>,
                       clobbered: &mut Vec<String>| {
        for op in frame.patch().ops() {
            let Op::Bind { id, prop, bind } = op else { continue };
            match (prop, bind) {
                (Prop::OffsetX | Prop::OffsetY, Bind::Track { .. }) => {
                    driven.insert(*id);
                }
                (Prop::OffsetX | Prop::OffsetY, Bind::Stop) => {
                    driven.remove(id);
                }
                (Prop::Offset, Bind::Set(_)) if driven.contains(id) => {
                    clobbered.push(format!("{id:?}"));
                }
                _ => {}
            }
        }
    };
    let mount = rig.mount(|ui| {
        scroll(ui, |ui| {
            for row in 0..60 {
                text(ui, format!("row {row}")).height(Len::dip(24.0));
            }
        })
        .width(Len::dip(300.0))
        .height(Len::dip(200.0));
    });
    inspect(&mount, &mut driven, &mut clobbered);
    drop(mount);
    assert!(!driven.is_empty(), "the container bound nothing to its tracker");
    // Several passes and a resize: the thumb's box is republished whenever the extents move,
    // which is where the composite used to come back.
    for _ in 0..3 {
        let frame = rig.flush();
        inspect(&frame, &mut driven, &mut clobbered);
        drop(frame);
    }
    for (w, h) in [(300.0, 260.0), (420.0, 200.0), (300.0, 200.0)] {
        let frame = rig.resize(w, h);
        inspect(&frame, &mut driven, &mut clobbered);
        drop(frame);
    }
    assert!(
        clobbered.is_empty(),
        "the composite offset was written over a driven axis on {clobbered:?}"
    );
}
