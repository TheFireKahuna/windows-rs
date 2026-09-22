//! The value path, driven against a real compositor.
//!
//! The rig mints its own nodes and publishes its own hit array rather than mounting an
//! application tree: what is under test is the front thread's writer, and a scene is the only
//! part of the rest of the system it actually writes through.

use super::*;
use crate::gesture::{DragUpdate, Manip};
use crate::input::{KeyEvent, Mods, PointerFlags, PointerType, Sample};
use crate::uia::Action;
use crate::widget::roles::{ScalarPart, TURN_SWEEP};
use windows_color::{DisplayCapability, OutputTransform};
use windows_scene::{
    BackdropSpec, Backends, ContactKind, Env, HitEntry, HitFlags, Ids, NO_ENTRY, NODE, NodeKind,
    Op, Point, Scene, SinkPatch,
};

/// A window, a scene and the two id authorities, so a test drives the production writer against
/// a real compositor rather than against a stand-in for one.
pub(super) struct Rig {
    pub controls: Controls,
    pub ids: Ids<CONTROL>,
    scene: Scene,
    back: Backends,
    env: Env,
    nodes: Ids<NODE>,
    patch: SinkPatch,
    _window: windows_window::Window,
}

impl Rig {
    pub fn new(title: &str) -> Result<Self> {
        windows_window::ensure_dispatcher_queue(windows_window::Apartment::Asta)?;
        let env = Env::new(
            96.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        );
        let window = windows_window::Window::new(title)
            .size_dips(800.0, 600.0)
            .create()?;
        let back = Backends::new(
            windows_composition::Compositor::new()?,
            &windows_d2d::Gpu::for_window()?,
            windows_text::FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]),
        )?;
        let scene = Scene::new_at(window.handle(), &back, env, BackdropSpec::default())?;
        let mut nodes = Ids::default();
        // Both halves seat the root at the first id without exchanging it, so the first mint
        // here would name a node the scene already holds.
        let _root = nodes.mint();
        Ok(Self {
            controls: Controls::new(),
            ids: Ids::default(),
            scene,
            back,
            env,
            nodes,
            patch: SinkPatch::default(),
            _window: window,
        })
    }

    /// Mints a group under the content band and returns it, applied immediately.
    pub fn node(&mut self) -> Result<NodeId> {
        let id = self.nodes.mint();
        self.patch.push(Op::New {
            id,
            kind: NodeKind::Group,
            parent: windows_scene::Attach::Window,
            after: None,
        });
        self.apply()?;
        Ok(id)
    }

    /// Replaces the hit array, which is what a pointer and the focus ring resolve through.
    pub fn publish_hits(&mut self, entries: &[HitEntry]) -> Result<()> {
        self.patch.hits_mut().extend_from_slice(entries);
        let mut index: Vec<_> = entries
            .iter()
            .enumerate()
            .map(|(at, entry)| (entry.id, at as u32))
            .collect();
        index.sort_unstable_by_key(|&(id, _)| id);
        self.patch.index_mut().extend_from_slice(&index);
        let (entries, index) = (self.patch.hits_span(), self.patch.index_span());
        self.patch.push(Op::Hits { entries, index });
        self.apply()
    }

    fn apply(&mut self) -> Result<()> {
        self.scene.apply(&mut self.patch, &self.back, self.env)?;
        Ok(())
    }

    pub fn animations(&self) -> u64 {
        self.scene.census().animations
    }

    pub fn visuals_minted(&self) -> u64 {
        self.scene.census().visuals_minted
    }

    /// Applies one tick against this rig's own scene.
    pub fn tick(&mut self, reports: &[Report], out: &mut Vec<Intent>) -> Result<()> {
        let Self {
            controls,
            scene,
            back,
            env,
            ..
        } = self;
        let mut front = Front {
            scene,
            back,
            env: *env,
        };
        controls.tick(reports, &mut front, out)
    }

    /// Adopts rows the way a patch does, through the one call the driver makes.
    pub fn adopt(
        &mut self,
        chrome: &[(ControlId, ChromeRow)],
        values: &[(ControlId, ValueRow)],
        released: &[ControlId],
    ) -> Result<()> {
        let Self {
            controls,
            scene,
            back,
            env,
            ..
        } = self;
        let mut front = Front {
            scene,
            back,
            env: *env,
        };
        controls.adopt(chrome, values, released, &mut front)
    }

    /// Applies automation actions against this rig's own scene.
    pub fn automation(&mut self, actions: &[Action], out: &mut Vec<Intent>) -> Result<()> {
        let Self {
            controls,
            scene,
            back,
            env,
            ..
        } = self;
        let mut front = Front {
            scene,
            back,
            env: *env,
        };
        controls.automation(actions, &mut front, out)
    }
}

/// An interactive entry over the box `(x0, y0)`–`(x1, y1)`, with no ancestry.
pub(super) fn entry(id: ControlId, x0: f32, y0: f32, x1: f32, y1: f32) -> HitEntry {
    HitEntry {
        x0,
        y0,
        x1,
        y1,
        touch_inflate: 0.0,
        clip_parent: NO_ENTRY,
        parent: NO_ENTRY,
        flags: HitFlags::INTERACTIVE,
        scroll_src: NodeId::NONE,
        id,
    }
}

pub(super) fn press_at(target: ControlId, at: Point) -> Report {
    Report::Pressed {
        target,
        contact: 1,
        buttons: 1,
        sample: Sample {
            id: 1,
            ptype: PointerType::Mouse,
            flags: PointerFlags(0),
            at,
            raw: at,
            contact: (0.0, 0.0),
            pen: None,
            time: 0,
            qpc: 0,
        },
    }
}

pub(super) fn press(target: ControlId) -> Report {
    press_at(target, Point::default())
}

fn moved(target: ControlId, at: Point) -> Report {
    let Report::Pressed { sample, .. } = press_at(target, at) else {
        unreachable!("the constructor above builds exactly one variant")
    };
    Report::Moved {
        target,
        contact: 1,
        sample,
    }
}

fn dragged(target: ControlId, phase: Phase, decided: bool) -> Report {
    Report::Dragged {
        target,
        contact: 1,
        update: DragUpdate {
            phase,
            delta: Point { x: 0.0, y: 12.0 },
            from: Point::default(),
            at: Point { x: 0.0, y: 12.0 },
            decided,
        },
    }
}

/// A slider row over a 100-DIP rail inset 13 DIPs into a 126-DIP hit box, snapping to a tenth.
fn slider_row(thumb: NodeId) -> ValueRow {
    ValueRow {
        parts: [
            (thumb, ScalarPart::Thumb { vertical: false }),
            (NodeId::NONE, ScalarPart::None),
            (NodeId::NONE, ScalarPart::None),
            (NodeId::NONE, ScalarPart::None),
        ],
        min: -24.0,
        span: 48.0,
        rest: 13.0,
        travel: 100.0,
        fraction: 0.5,
        step: 0.1,
        revision: 0,
    }
}

fn committed(out: &[Intent]) -> usize {
    out.iter()
        .filter(|i| matches!(i.what, What::Scalar { commit: true, .. }))
        .count()
}

#[test]
fn native_an_idle_scalar_adopts_another_controls_edit_in_the_same_document() -> Result<()> {
    let mut rig = Rig::new("shared scalar source")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    let row = slider_row(thumb);
    rig.adopt(&[], &[(id, row)], &[])?;
    let edited = ValueRow { fraction: 0.75, ..row };
    rig.adopt(&[], &[(id, edited)], &[])?;
    assert_eq!(rig.controls.value(id).fraction, 0.75);

    let mut out = Vec::new();
    rig.automation(&[Action::SetValue(id, -24.0)], &mut out)?;
    rig.adopt(&[], &[(id, ValueRow { travel: 200.0, ..edited })], &[])?;
    assert_eq!(rig.controls.value(id).fraction, 0.0,
        "unchanged source values must preserve a pending input through layout");
    Ok(())
}

#[test]
fn native_one_writer_carries_every_value_and_a_changed_revision_supersedes_a_gesture() -> Result<()>
{
    let mut rig = Rig::new("scalar ownership")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 126.0, 32.0)])?;
    let chrome = ChromeRow {
        flags: flag::SLIDE,
        ..ChromeRow::default()
    };
    let row = slider_row(thumb);
    rig.adopt(&[(id, chrome)], &[(id, row)], &[])?;

    // A mount arrives carried, so the thumb is placed rather than sprung to zero.
    assert_eq!(rig.animations(), 0, "an adopted value must snap");
    let minted = rig.visuals_minted();

    let mut out = Vec::with_capacity(32);
    // The pointer lands 13 DIPs in, which is the rail's own origin: the value is the floor.
    rig.tick(&[press(id), moved(id, Point { x: 13.0, y: 8.0 })], &mut out)?;
    assert_eq!(
        out.iter()
            .filter_map(|i| match i.what {
                What::Scalar { value, .. } => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [-24.0, -24.0],
        "the hit box's half-thumb gutters are outside the value range"
    );
    // The press sprang and the move carried, which is the whole of the two platform facts.
    assert_eq!(rig.animations(), 1);
    out.clear();

    rig.tick(&[moved(id, Point { x: 63.0, y: 8.0 })], &mut out)?;
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].what, What::Scalar { value, commit: false, .. } if value == 0.0));
    out.clear();

    // A release at the far gutter settles at the ceiling and commits exactly once.
    rig.tick(
        &[Report::Released {
            target: id,
            contact: 1,
            at: Point { x: 140.0, y: 8.0 },
        }],
        &mut out,
    )?;
    assert_eq!(committed(&out), 1);
    assert!(matches!(out[0].what, What::Scalar { value, .. } if value == 24.0));
    out.clear();

    // A geometry-only update repeats the revision, so the pointer's fraction stands.
    rig.adopt(
        &[(id, chrome)],
        &[(
            id,
            ValueRow {
                rest: 20.0,
                travel: 180.0,
                ..row
            },
        )],
        &[],
    )?;
    rig.tick(&[], &mut out)?;
    assert!(out.is_empty(), "adopting geometry raises nothing");
    rig.tick(&[press(id)], &mut out)?;
    out.clear();

    // A changed revision under a live contact adopts the application's value and supersedes
    // the gesture standing on it, which is raised on the next tick.
    rig.adopt(
        &[(id, chrome)],
        &[(
            id,
            ValueRow {
                revision: 1,
                fraction: 0.25,
                ..row
            },
        )],
        &[],
    )?;
    rig.tick(&[], &mut out)?;
    assert_eq!(
        out,
        [Intent {
            target: id,
            what: What::Canceled(1)
        }]
    );
    out.clear();

    // A key, a detent and an automation set reach the same writer, so each snaps the same way
    // and commits exactly once.
    let mut front_out = Vec::new();
    rig.automation(&[Action::SetValue(id, 2.0)], &mut front_out)?;
    rig.tick(
        &[
            Report::Key {
                target: Some(id),
                event: KeyEvent {
                    kind: KeyKind::Down,
                    key: 0x24,
                    repeat: false,
                    mods: Mods::default(),
                },
            },
            Report::Rotary {
                target: Some(id),
                degrees: 10.0,
                steps: 1.0,
            },
        ],
        &mut front_out,
    )?;
    // The rotary arm needs the turn flag, so only the key and the automation set land here.
    assert_eq!(committed(&front_out), 2);
    assert!(front_out.iter().all(|i| matches!(
        i.what,
        What::Scalar { value, .. } if (-24.0..=24.0).contains(&value)
    )));
    assert_eq!(
        rig.visuals_minted(),
        minted,
        "the value path mints no visual"
    );
    Ok(())
}

#[test]
fn native_a_turn_reads_the_rotation_about_its_own_centre_and_a_cancel_restores_it() -> Result<()> {
    let mut rig = Rig::new("knob rotation")?;
    let needle = rig.node()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 48.0, 48.0)])?;
    let chrome = ChromeRow {
        flags: flag::TURN,
        ..ChromeRow::default()
    };
    let row = ValueRow {
        parts: [
            (
                needle,
                ScalarPart::Rotation {
                    from: 0.0,
                    to: TURN_SWEEP,
                },
            ),
            (NodeId::NONE, ScalarPart::None),
            (NodeId::NONE, ScalarPart::None),
            (NodeId::NONE, ScalarPart::None),
        ],
        min: 0.0,
        span: 1.0,
        rest: 0.0,
        travel: 0.0,
        fraction: 0.5,
        step: 0.0,
        revision: 0,
    };
    rig.adopt(&[(id, chrome)], &[(id, row)], &[])?;

    let mut out = Vec::with_capacity(16);
    rig.tick(&[press(id)], &mut out)?;
    assert!(out.is_empty(), "a press on a turned control moves nothing");
    // A quarter of the sweep clockwise, in the degrees the platform reports.
    let quarter = (TURN_SWEEP * 0.25).to_degrees();
    rig.tick(
        &[Report::Gesture {
            target: id,
            contact: 1,
            event: Recognised::ManipulationUpdated {
                at: Point::default(),
                delta: Manip::default(),
                cumulative: Manip {
                    rotation: quarter,
                    ..Manip::default()
                },
            },
        }],
        &mut out,
    )?;
    assert_eq!(out.len(), 1);
    assert!(
        matches!(out[0].what, What::Scalar { value, .. } if (value - 0.75).abs() < 1e-5),
        "a turn is a displacement from the fraction the contact landed on"
    );
    out.clear();

    // A contact taken away puts the value back where it found it and commits nothing.
    rig.tick(
        &[Report::Canceled {
            target: id,
            contact: 1,
        }],
        &mut out,
    )?;
    assert_eq!(committed(&out), 0);
    assert!(matches!(
        out.last().expect("a cancel is reported").what,
        What::Canceled(0)
    ));
    assert!(
        out.iter()
            .any(|i| matches!(i.what, What::Scalar { value, .. } if (value - 0.5).abs() < 1e-5))
    );
    Ok(())
}

#[test]
fn native_a_canceled_decided_drag_raises_exactly_one_report() -> Result<()> {
    let mut rig = Rig::new("drag cancellation")?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 200.0, 32.0)])?;
    let chrome = ChromeRow {
        flags: flag::DRAGS,
        ..ChromeRow::default()
    };
    rig.adopt(&[(id, chrome)], &[], &[])?;

    let mut out = Vec::with_capacity(8);
    rig.tick(
        &[
            press(id),
            dragged(id, Phase::Vertical, true),
            // A locked drag reports zero on the axis it does not own, so a later sample can
            // look undecided; the lock is never revisited and neither is this.
            dragged(id, Phase::Undecided, false),
            Report::Canceled {
                target: id,
                contact: 1,
            },
        ],
        &mut out,
    )?;
    assert_eq!(
        out.iter()
            .filter(|i| matches!(
                i.what,
                What::DragEnded(_) | What::Canceled(_) | What::Tapped
            ))
            .count(),
        1,
        "a canceled decided drag ends once and is neither a tap nor a separate cancel"
    );
    assert_eq!(out.last().map(|i| i.what), Some(What::DragEnded(None)));
    out.clear();

    // Below the threshold a drag has no meaning, so a nudge while clicking is still a click.
    rig.tick(
        &[
            press(id),
            dragged(id, Phase::Undecided, false),
            Report::Released {
                target: id,
                contact: 1,
                at: Point::default(),
            },
        ],
        &mut out,
    )?;
    assert_eq!(out.last().map(|i| i.what), Some(What::Tapped));
    Ok(())
}

#[test]
fn native_a_warm_gesture_allocates_nothing() -> Result<()> {
    let mut rig = Rig::new("scalar allocation")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 126.0, 32.0)])?;
    rig.adopt(
        &[(
            id,
            ChromeRow {
                flags: flag::SLIDE,
                ..ChromeRow::default()
            },
        )],
        &[(id, slider_row(thumb))],
        &[],
    )?;
    let mut out = Vec::with_capacity(32);
    rig.tick(&[press(id), moved(id, Point { x: 40.0, y: 8.0 })], &mut out)?;

    // This counts Rust allocations on this thread, not allocations inside Windows.
    let before = crate::counting::allocations();
    let animations = rig.animations();
    for step in 0..1_000 {
        out.clear();
        let x = if step % 2 == 0 { 40.0 } else { 80.0 };
        rig.tick(&[moved(id, Point { x, y: 8.0 })], &mut out)?;
    }
    assert_eq!(
        crate::counting::allocations() - before,
        0,
        "warm scalar retargeting allocated"
    );
    assert_eq!(
        rig.animations(),
        animations,
        "a carried value is a write and not a spring"
    );
    Ok(())
}

#[test]
fn native_retirement_takes_a_control_out_of_every_slot_it_is_held_in() -> Result<()> {
    let mut rig = Rig::new("control lifetime")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 126.0, 32.0)])?;
    rig.adopt(
        &[(
            id,
            ChromeRow {
                flags: flag::SLIDE | flag::DRAGS,
                ..ChromeRow::default()
            },
        )],
        &[(id, slider_row(thumb))],
        &[],
    )?;
    let mut out = Vec::new();
    rig.tick(
        &[
            Report::HoverChanged {
                from: None,
                to: Some(id),
                at: Point::default(),
                qpc: 0,
            },
            press(id),
            dragged(id, Phase::Vertical, true),
        ],
        &mut out,
    )?;
    out.clear();

    rig.adopt(&[], &[], &[id])?;
    // A late report about a retired control reaches nothing, and the hit array still names it,
    // so the miss is the table's own and not the router's.
    rig.tick(
        &[
            Report::Released {
                target: id,
                contact: 1,
                at: Point::default(),
            },
            Report::Canceled {
                target: id,
                contact: 1,
            },
        ],
        &mut out,
    )?;
    assert!(
        out.is_empty(),
        "a retired generation cannot move a visual or raise an intent"
    );
    // The scene still answers for the point, which is what makes the miss a table decision.
    assert!(
        rig.scene
            .hit(Point { x: 10.0, y: 10.0 }, ContactKind::Mouse)
            .is_some()
    );
    Ok(())
}
