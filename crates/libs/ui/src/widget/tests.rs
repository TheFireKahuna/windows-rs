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
use crate::layout::{Len, Preset};
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
    assert_eq!(live.get(), Some(0.0625), "the cell did not take the declared value");
    rig.set(gain, 2.0);
    assert_eq!(live.get(), Some(0.5), "the cell did not follow the source");
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
