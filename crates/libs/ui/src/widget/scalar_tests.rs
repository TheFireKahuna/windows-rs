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
        self.node_in(windows_scene::Attach::Window)
    }

    /// Mints a detached group in the overlay band.
    pub fn overlay_node(&mut self) -> Result<NodeId> {
        self.node_in(windows_scene::Attach::Overlay)
    }

    fn node_in(&mut self, parent: windows_scene::Attach) -> Result<NodeId> {
        let id = self.nodes.mint();
        self.patch.push(Op::New {
            id,
            kind: NodeKind::Group,
            parent,
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

    pub fn correlate(&mut self, routes: &mut crate::correlation::Routes) -> Result<()> {
        let mut front = Front { scene: &mut self.scene, back: &self.back, env: self.env };
        routes.reveal(&mut front)
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

    pub fn adopt_translations(&mut self, rows: &[(ControlId, NodeId, windows_scene::Translation)],
        released: &[ControlId]) -> Result<()> {
        let mut front = Front { scene: &mut self.scene, back: &self.back, env: self.env };
        self.controls.adopt_translations(rows, released, &mut front)
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

#[test]
fn native_correlation_retargets_retained_members_without_idle_work() -> Result<()> {
    use crate::correlation::{Correlation, Member, Route, Router, Routes};
    use crate::present::Live;
    use windows_present::SubId;

    let mut rig = Rig::new("correlation reveal")?;
    let live = Live::new()?;
    let group = Correlation::new(&live);
    let (a, b, region) = (rig.ids.mint(), rig.ids.mint(), rig.ids.mint());
    let (ra, rb) = (rig.node()?, rig.node()?);
    rig.publish_hits(&[entry(a, 0.0, 0.0, 100.0, 32.0),
        entry(b, 100.0, 0.0, 200.0, 32.0), entry(region, 0.0, 32.0, 200.0, 132.0)])?;
    let rows = [
        Route { source: a, member: Member { group: group.clone(), key: Some(SubId(1)), reveal: ra } },
        Route { source: b, member: Member { group: group.clone(), key: Some(SubId(2)), reveal: rb } },
        Route { source: region, member: Member { group: group.clone(), key: None, reveal: NodeId::NONE } },
    ];
    let mut routes = Routes::default();
    routes.adopt(&rows, &[]);
    let mut router = Router::default();
    router.sync(&rows, rig.scene.hits());
    let before = rig.animations();
    router.route(&[moved(a, Point::default())], rig.scene.hits());
    rig.correlate(&mut routes)?;
    assert_eq!(rig.animations() - before, 1);
    live.input.set_hover(Some(SubId(2)));
    router.route(&[moved(region, Point::default())], rig.scene.hits());
    rig.correlate(&mut routes)?;
    assert_eq!(rig.animations() - before, 3, "the previous member hides and the next reveals");
    let settled = rig.animations();
    let minted = rig.visuals_minted();
    let allocations = crate::counting::allocations();
    for _ in 0..32 {
        assert!(!router.route(&[moved(region, Point::default())], rig.scene.hits()));
        rig.correlate(&mut routes)?;
    }
    assert_eq!(rig.animations(), settled);
    assert_eq!(rig.visuals_minted(), minted);
    assert_eq!(crate::counting::allocations(), allocations);
    for _ in 0..2 {
        router.route(&[moved(a, Point::default())], rig.scene.hits());
        rig.correlate(&mut routes)?;
        router.route(&[moved(b, Point::default())], rig.scene.hits());
        rig.correlate(&mut routes)?;
    }
    let allocations = crate::counting::allocations();
    for _ in 0..32 {
        router.route(&[moved(a, Point::default())], rig.scene.hits());
        rig.correlate(&mut routes)?;
        router.route(&[moved(b, Point::default())], rig.scene.hits());
        rig.correlate(&mut routes)?;
    }
    assert_eq!(crate::counting::allocations(), allocations);
    assert_eq!(rig.visuals_minted(), minted);
    routes.adopt(&[], &[b]);
    router.sync(&[rows[0].clone(), rows[2].clone()], rig.scene.hits());
    let retired = rig.animations();
    rig.correlate(&mut routes)?;
    assert_eq!(rig.animations(), retired, "retirement does not address the released target");
    assert_eq!(group.selected(), None);
    Ok(())
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
        animate_layout: false,
        extent: None,
        fraction: 0.5,
        step: 4.8,
        revision: 0,
    }
}

fn committed(out: &[Intent]) -> usize {
    out.iter()
        .filter(|i| matches!(i.what, What::Scalar { commit: true, .. }))
        .count()
}

#[test]
fn native_automation_preserves_explicit_disclosure_states() -> Result<()> {
    let mut rig = Rig::new("automation expansion")?;
    let id = rig.ids.mint();
    rig.adopt(&[(id, ChromeRow::default())], &[], &[])?;
    let mut out = Vec::new();
    rig.automation(&[Action::Expand(id, false), Action::Expand(id, true), Action::Expand(id, true)], &mut out)?;
    assert_eq!(out, [
        Intent { target: id, what: What::Expanded(false) },
        Intent { target: id, what: What::Expanded(true) },
        Intent { target: id, what: What::Expanded(true) },
    ]);
    Ok(())
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
fn native_scalar_geometry_uses_layout_motion_without_restarting_on_republication() -> Result<()> {
    let mut rig = Rig::new("scalar layout motion")?;
    let thumb = rig.node()?;
    let id = rig.ids.mint();
    let row = ValueRow { animate_layout: true, ..slider_row(thumb) };
    rig.adopt(&[(id, ChromeRow { flags: flag::SLIDE, ..ChromeRow::default() })], &[(id, row)], &[])?;
    assert_eq!(rig.animations(), 0);
    let resized = ValueRow { travel: 200.0, ..row };
    rig.adopt(&[], &[(id, resized)], &[])?;
    assert_eq!(rig.animations(), 1);
    rig.adopt(&[], &[(id, resized)], &[])?;
    assert_eq!(rig.animations(), 1);
    rig.adopt(&[], &[(id, row)], &[])?;
    assert_eq!(rig.animations(), 2);
    assert_eq!(rig.controls.value(id).fraction, 0.5);
    rig.adopt(&[], &[(id, ValueRow { animate_layout: false, ..resized })], &[])?;
    let settled = rig.animations();
    rig.scene.set_springs_enabled(false);
    rig.adopt(&[], &[(id, row)], &[])?;
    assert_eq!(rig.animations(), settled);
    Ok(())
}

#[test]
fn native_toggle_updates_spring_but_mount_resize_and_republication_do_not() -> Result<()> {
    let mut rig = Rig::new("toggle source motion")?;
    let thumb = rig.node()?;
    let track = rig.node()?;
    let id = rig.ids.mint();
    let off = ValueRow {
        parts: [
            (thumb, ScalarPart::Thumb { vertical: false }),
            (track, ScalarPart::Fade),
            (NodeId::NONE, ScalarPart::None),
            (NodeId::NONE, ScalarPart::None),
        ],
        rest: 2.0,
        travel: 12.0,
        ..ValueRow::default()
    };
    rig.adopt(&[(id, ChromeRow::default())], &[(id, off)], &[])?;
    assert_eq!(rig.animations(), 1, "mount binds the colour follower without springing");
    let minted = rig.visuals_minted();
    let on = ValueRow { fraction: 1.0, revision: 1, ..off };
    rig.adopt(&[], &[(id, on)], &[])?;
    assert_eq!(rig.animations(), 2, "a model toggle must start only the thumb spring");
    rig.adopt(&[], &[(id, on)], &[])?;
    assert_eq!(rig.animations(), 2, "republication must not restart the spring or follower");
    rig.adopt(&[], &[(id, off)], &[])?;
    assert_eq!(rig.animations(), 3, "reversal must retarget only the thumb spring");
    let resized = ValueRow { travel: 14.0, ..off };
    rig.adopt(&[], &[(id, resized)], &[])?;
    assert_eq!(rig.animations(), 6, "resize rebinds the follower and settles both thumb axes");
    rig.scene.set_springs_enabled(false);
    rig.adopt(&[], &[(id, ValueRow { fraction: 1.0, revision: 1, ..resized })], &[])?;
    assert_eq!(rig.animations(), 6, "reduced motion must bypass the spring and retain the follower");
    assert_eq!(rig.visuals_minted(), minted);
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
    // Cancelling the press spring needs one finite settlement per offset axis.
    assert_eq!(rig.animations(), 3);
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
        animate_layout: false,
        extent: None,
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
fn native_landing_retires_on_completion_and_is_canceled_by_a_new_preview() -> Result<()> {
    let mut rig = Rig::new("reorder landing")?;
    let parent = rig.node()?;
    let tile = rig.node_in(windows_scene::Attach::Node(parent))?;
    rig.patch.push(Op::Bind { id: tile, prop: Prop::Size,
        bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
    rig.apply()?;
    let count = rig.scene.census().visuals_live;
    rig.scene.land_drag_preview(tile, Vector2::new(120.0, 50.0), &rig.back)?;
    let epoch = rig.scene.drag_preview_epoch().unwrap();
    assert_eq!(rig.scene.census().visuals_live, count + 1);
    assert_eq!(rig.scene.hits().lifted_owner(), ControlId::NONE);
    rig.back.request_commit()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut events = Vec::new();
    while rig.scene.drag_preview_epoch().is_some() {
        assert!(std::time::Instant::now() < deadline, "landing completion did not restore the tile");
        windows_window::pump();
        rig.scene.drain_events(&mut events);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(rig.scene.census().visuals_live, count);
    let settled = *rig.scene.census();
    for _ in 0..100 { rig.scene.drain_events(&mut events); }
    assert_eq!(*rig.scene.census(), settled);
    rig.scene.land_drag_preview(tile, Vector2::new(-80.0, 0.0), &rig.back)?;
    assert!(rig.scene.drag_preview_epoch().unwrap() > epoch);
    assert!(rig.scene.begin_drag_preview(tile, ControlId::NONE, &rig.back));
    let dragging = rig.scene.drag_preview_epoch();
    rig.scene.finish_drag_preview(epoch);
    assert_eq!(rig.scene.drag_preview_epoch(), dragging);
    rig.scene.end_drag_preview();
    assert_eq!(rig.scene.census().visuals_live, count);
    rig.scene.set_springs_enabled(false);
    rig.scene.land_drag_preview(tile, Vector2::new(100.0, 30.0), &rig.back)?;
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    rig.scene.set_springs_enabled(true);
    rig.scene.land_drag_preview(tile, Vector2::zero(), &rig.back)?;
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    rig.scene.land_drag_preview(tile, Vector2::new(100.0, 0.0), &rig.back)?;
    rig.patch.push(Op::Drop { id: tile, exit: windows_scene::Exit::None,
        origin: Vector2::zero(), bounds: None });
    rig.apply()?;
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    assert_eq!(rig.scene.census().visuals_live, count - 1);
    Ok(())
}

#[test]
fn native_drag_preview_lifts_once_restores_on_cancel_and_ignores_late_reports() -> Result<()> {
    let mut rig = Rig::new("drag preview")?;
    let parent = rig.node()?;
    let tile = rig.node_in(windows_scene::Attach::Node(parent))?;
    rig.patch.push(Op::Bind { id: tile, prop: Prop::Size,
        bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
    rig.apply()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 80.0, 40.0)])?;
    rig.adopt(&[(id, ChromeRow { flags: flag::DRAGS | flag::DRAG_PREVIEW,
        ..ChromeRow::default() })], &[], &[])?;
    rig.controls.adopt_previews(&[(id, tile)]);
    let count = rig.scene.census().visuals_live;
    let mut out = Vec::with_capacity(8);
    rig.tick(&[press(id), dragged(id, Phase::Undecided, false)], &mut out)?;
    assert_eq!(rig.scene.census().visuals_live, count);
    rig.tick(&[dragged(id, Phase::Free, true)], &mut out)?;
    assert_eq!(rig.scene.census().visuals_live, count + 1);
    let minted = rig.visuals_minted();
    let allocations = crate::counting::allocations();
    for _ in 0..100 {
        out.clear();
        rig.tick(&[dragged(id, Phase::Free, false)], &mut out)?;
    }
    assert_eq!(crate::counting::allocations(), allocations);
    assert_eq!(rig.visuals_minted(), minted);
    rig.tick(&[Report::Canceled { target: id, contact: 1 }], &mut out)?;
    assert_eq!(rig.scene.census().visuals_live, count);
    assert_eq!(out.last().map(|intent| intent.what), Some(What::DragEnded(None)));
    out.clear();
    rig.tick(&[dragged(id, Phase::Free, true)], &mut out)?;
    assert!(out.is_empty());
    assert_eq!(rig.scene.census().visuals_live, count);
    rig.tick(&[press(id), dragged(id, Phase::Free, true)], &mut out)?;
    rig.adopt(&[], &[], &[id])?;
    assert_eq!(rig.scene.census().visuals_live, count);
    assert!(rig.controls.previews.is_empty());
    let stopped = *rig.scene.census();
    rig.tick(&[], &mut out)?;
    assert_eq!(*rig.scene.census(), stopped);
    Ok(())
}

#[test]
fn native_released_preview_waits_for_its_ack_and_rejects_old_gestures() -> Result<()> {
    let mut rig = Rig::new("released drag preview")?;
    let parent = rig.node()?;
    let tile = rig.node_in(windows_scene::Attach::Node(parent))?;
    rig.patch.push(Op::Bind { id: tile, prop: Prop::Size,
        bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
    rig.apply()?;
    let id = rig.ids.mint();
    rig.publish_hits(&[entry(id, 0.0, 0.0, 80.0, 40.0)])?;
    rig.adopt(&[(id, ChromeRow { flags: flag::DRAGS | flag::DRAG_PREVIEW,
        ..ChromeRow::default() })], &[], &[])?;
    rig.controls.adopt_previews(&[(id, tile)]);
    let count = rig.scene.census().visuals_live;
    let mut out = Vec::with_capacity(8);
    let release = Report::Released { target: id, contact: 1, at: Point { x: 24.0, y: 0.0 } };
    rig.tick(&[press(id), dragged(id, Phase::Free, true), release], &mut out)?;
    let first = rig.controls.take_preview_release().expect("released preview identity");
    assert_eq!(rig.controls.take_preview_release(), None);
    assert_eq!(rig.scene.drag_preview_epoch(), Some(first));
    assert_eq!(rig.scene.census().visuals_live, count + 1);
    assert!(matches!(out.last().map(|i| i.what), Some(What::DragEnded(Some(_)))));
    let held = *rig.scene.census();
    out.clear();
    rig.tick(&[], &mut out)?;
    assert_eq!(*rig.scene.census(), held);
    assert!(out.is_empty());
    rig.apply()?;
    assert_eq!(rig.scene.drag_preview_epoch(), Some(first));
    rig.scene.finish_drag_preview(first);
    assert_eq!(rig.scene.census().visuals_live, count);

    rig.tick(&[press(id), dragged(id, Phase::Free, true), release], &mut out)?;
    let second = rig.controls.take_preview_release().unwrap();
    assert!(second > first);
    rig.tick(&[press(id), dragged(id, Phase::Free, true)], &mut out)?;
    let third = rig.scene.drag_preview_epoch().unwrap();
    assert!(third > second);
    rig.scene.finish_drag_preview(second);
    assert_eq!(rig.scene.drag_preview_epoch(), Some(third));
    assert_eq!(rig.scene.census().visuals_live, count + 1);
    rig.tick(&[Report::Canceled { target: id, contact: 1 }], &mut out)?;
    assert_eq!(rig.controls.take_preview_release(), None);
    assert_eq!(rig.scene.census().visuals_live, count);

    out.clear();
    rig.tick(&[press(id), dragged(id, Phase::Free, true), release], &mut out)?;
    let retired = rig.controls.take_preview_release().unwrap();
    rig.patch.push(Op::Drop { id: tile, exit: windows_scene::Exit::None,
        origin: Vector2::zero(), bounds: None });
    rig.apply()?;
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    let after_drop = *rig.scene.census();
    rig.scene.finish_drag_preview(retired);
    assert_eq!(*rig.scene.census(), after_drop);
    assert_eq!(rig.scene.census().visuals_live, count - 1);
    Ok(())
}

#[test]
fn native_grid_reorder_displaces_neighbors_on_index_edges_and_cancels_stale_geometry() -> Result<()> {
    let mut rig = Rig::new("scene grid reorder")?;
    let group = rig.node()?;
    let mut rows = Vec::new();
    let mut hits = Vec::new();
    for (index, (x, y)) in [(0.0, 0.0), (100.0, 0.0), (0.0, 60.0)].into_iter().enumerate() {
        let node = rig.node_in(windows_scene::Attach::Node(group))?;
        rig.patch.push(Op::Bind { id: node, prop: Prop::Size,
            bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
        for prop in [Prop::TranslationX, Prop::TranslationY] {
            rig.patch.push(Op::Bind { id: node, prop, bind: Bind::Set(Value::Scalar(0.0)) });
        }
        let id = rig.ids.mint();
        hits.push(entry(id, x, y, x + 80.0, y + 40.0));
        rows.push(ReorderRow { id, node, group, index: index as u32,
            state: windows_scene::Translation::new(Vector2::zero()) });
    }
    rig.apply()?;
    rig.publish_hits(&hits)?;
    let chrome: Vec<_> = rows.iter().map(|row| (row.id, ChromeRow {
        flags: flag::DRAGS | flag::DRAG_PREVIEW, ..ChromeRow::default()
    })).collect();
    rig.adopt(&chrome, &[], &[])?;
    rig.controls.adopt_previews(&rows.iter().map(|r| (r.id, r.node)).collect::<Vec<_>>());
    rig.controls.adopt_reorders(&rows, &[]);
    rig.controls.adopt_translations(&rows.iter().map(|r| (r.id, r.node, r.state.clone())).collect::<Vec<_>>(), &[],
        &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    let id = rows[0].id;
    let sample = |x, y, decided| Report::Dragged { target: id, contact: 1,
        update: DragUpdate { phase: Phase::Free, from: Point::default(), at: Point { x, y },
            delta: Point { x, y }, decided } };
    let mut out = Vec::with_capacity(8);
    let resting_visuals = rig.scene.census().visuals_live;
    rig.tick(&[press(id), sample(70.0, 80.0, true)], &mut out)?;
    assert_eq!(rig.scene.census().visuals_live, resting_visuals + 3);
    assert_eq!(rows[0].state.get(), Vector2::new(70.0, 80.0));
    assert_eq!(rig.scene.hits().shifted(0), [70.0, 80.0, 150.0, 120.0]);
    assert_eq!(rows[1].state.get(), Vector2::new(-100.0, 0.0));
    assert_eq!(rows[2].state.get(), Vector2::new(100.0, -60.0));
    assert_eq!(out.last().map(|i| i.what), Some(What::Reordered(ReorderUpdate { from: 0, to: 2, decided: true })));
    assert!(rig.controls.take_translation_changed());
    let animations = rig.animations();
    let minted = rig.visuals_minted();
    let allocations = crate::counting::allocations();
    for _ in 0..100 {
        out.clear();
        rig.tick(&[sample(70.0, 80.0, false)], &mut out)?;
        assert!(out.is_empty());
    }
    assert_eq!(crate::counting::allocations(), allocations);
    assert_eq!(rig.animations(), animations);
    assert_eq!(rig.visuals_minted(), minted);
    assert!(!rig.controls.take_translation_changed());
    assert_eq!(rig.scene.hits().shifted(1), [0.0, 0.0, 80.0, 40.0]);
    out.clear();
    rig.tick(&[sample(40.0, 80.0, false)], &mut out)?;
    assert!(out.is_empty(), "the central deadband retains insertion");
    assert_eq!(rows[0].state.get(), Vector2::new(40.0, 80.0));
    assert!(rig.controls.take_translation_changed());
    rig.tick(&[sample(10.0, 10.0, false)], &mut out)?;
    assert_eq!(rows[1].state.get(), Vector2::zero());
    assert_eq!(rows[2].state.get(), Vector2::zero());
    assert!(matches!(out.last().map(|i| i.what), Some(What::Reordered(ReorderUpdate { to: 0, .. }))));
    out.clear();
    rig.tick(&[Report::Released { target: id, contact: 1, at: Point { x: 70.0, y: 80.0 } }], &mut out)?;
    assert!(matches!(out.last().map(|i| i.what), Some(What::ReorderEnded(Some(ReorderUpdate { to: 2, .. })))));
    assert_eq!(rows[2].state.get(), Vector2::new(100.0, -60.0));
    let epoch = rig.controls.take_preview_release().unwrap();
    assert!(rig.controls.reorder_landing(epoch, ControlId::NONE, &rig.scene, &rig.patch).is_none());
    assert!(rig.controls.reorder_landing(epoch + 1, id, &rig.scene, &rig.patch).is_none());
    assert!(rig.controls.reorder_landing(epoch, id, &rig.scene, &rig.patch).is_some());
    rig.controls.finish_reorder(epoch, &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    rig.scene.finish_drag_preview(epoch);
    assert_eq!(rig.scene.census().visuals_live, resting_visuals);
    assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));
    for from in 0..rows.len() {
        for to in 0..rows.len() {
            let target = rows[from].id;
            let slot = hits[to];
            let point = Point { x: slot.x0 + (slot.x1 - slot.x0) * if to < from { 0.2 } else { 0.8 },
                y: (slot.y0 + slot.y1) * 0.5 };
            out.clear();
            rig.tick(&[press(target), Report::Dragged { target, contact: 1,
                update: DragUpdate { phase: Phase::Free, from: Point::default(), at: point,
                    delta: point, decided: true } }], &mut out)?;
            let mut order: Vec<_> = (0..rows.len()).collect();
            let source = order.remove(from);
            order.insert(to, source);
            for (slot, &original) in order.iter().enumerate() {
                if original == from { continue; }
                assert_eq!(rows[original].state.get(), Vector2::new(
                    hits[slot].x0 - hits[original].x0, hits[slot].y0 - hits[original].y0));
            }
            assert!(matches!(out.last().map(|i| i.what), Some(What::Reordered(update)) if update.to == to as u32));
            let current = rig.scene.drag_preview_epoch().unwrap();
            rig.controls.finish_reorder(epoch, &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
            assert_eq!(rig.scene.drag_preview_epoch(), Some(current));
            rig.tick(&[Report::Canceled { target, contact: 1 }], &mut out)?;
            assert_eq!(rig.scene.census().visuals_live, resting_visuals);
            assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));
            assert_eq!(out.last().map(|i| i.what), Some(What::ReorderEnded(None)));
        }
    }
    rig.tick(&[press(id), sample(70.0, 80.0, true)], &mut out)?;
    hits[2].y1 += 10.0;
    rig.publish_hits(&hits)?;
    out.clear();
    rig.tick(&[Report::Released { target: id, contact: 1, at: Point { x: 70.0, y: 80.0 } }], &mut out)?;
    assert_eq!(out.last().map(|i| i.what), Some(What::ReorderEnded(None)));
    assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    assert_eq!(rig.scene.census().visuals_live, resting_visuals);
    rig.tick(&[press(id), sample(70.0, 80.0, true)], &mut out)?;
    assert_eq!(rig.scene.census().visuals_live, resting_visuals + 3);
    rig.env = Env::new(144.0, rig.env.output());
    rig.apply()?;
    assert_eq!(rig.scene.drag_preview_epoch(), None);
    assert_eq!(rig.scene.census().visuals_live, resting_visuals);
    rig.tick(&[sample(70.0, 80.0, false)], &mut out)?;
    assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));
    Ok(())
}

#[test]
fn native_grid_reorder_autoscroll_keeps_source_and_drop_geometry_coherent() -> Result<()> {
    use crate::layout::{ScrollTable, Reveal, thumb_geom};
    use crate::seam::{ScrollFront, ScrollOp};
    use windows_scene::{Affine, Axes, GroupId, TrackerAxis, TrackerId, TrackerOp, TRACKER};
    let mut rig = Rig::new("reorder autoscroll")?;
    let viewport = rig.node()?;
    let group = rig.node_in(windows_scene::Attach::Node(viewport))?;
    let tracker = TrackerId::new(Ids::<TRACKER>::default().mint());
    rig.patch.push(Op::Bind { id: viewport, prop: Prop::Size,
        bind: Bind::Set(Value::Vec2(Vector2::new(100.0, 120.0))) });
    rig.patch.push(Op::Tracker { id: tracker.erased(), op: TrackerOp::Create {
        viewport: GroupId(viewport), axes: Axes::VERTICAL, owned: true } });
    rig.patch.push(Op::Tracker { id: tracker.erased(), op: TrackerOp::Bounds {
        min: Vector2::zero(), max: Vector2::new(0.0, 1000.0) } });
    rig.patch.push(Op::Bind { id: group, prop: Prop::OffsetY, bind: Bind::Track {
        tracker: tracker.erased(), axis: TrackerAxis::PositionY, affine: Affine::CONTENT } });
    rig.apply()?;
    let hover = rig.ids.mint();
    let mut hits = vec![entry(hover, 0.0, 0.0, 100.0, 120.0)];
    let mut rows = Vec::new();
    for index in 0..8 {
        let node = rig.node_in(windows_scene::Attach::Node(group))?;
        rig.patch.push(Op::Bind { id: node, prop: Prop::Size,
            bind: Bind::Set(Value::Vec2(Vector2::new(80.0, 40.0))) });
        let id = rig.ids.mint();
        let y = index as f32 * 60.0;
        let mut hit = entry(id, 0.0, y, 80.0, y + 40.0);
        hit.scroll_src = viewport;
        hit.clip_parent = 0;
        hits.push(hit);
        rows.push(ReorderRow { id, node, group, index,
            state: windows_scene::Translation::new(Vector2::zero()) });
    }
    rig.apply()?;
    rig.publish_hits(&hits)?;
    rig.adopt(&rows.iter().map(|row| (row.id, ChromeRow {
        flags: flag::DRAGS | flag::DRAG_PREVIEW, ..ChromeRow::default()
    })).collect::<Vec<_>>(), &[], &[])?;
    rig.controls.adopt_previews(&rows.iter().map(|r| (r.id, r.node)).collect::<Vec<_>>());
    rig.controls.adopt_reorders(&rows, &[]);
    rig.adopt_translations(&rows.iter().map(|r| (r.id, r.node, r.state.clone())).collect::<Vec<_>>(), &[])?;
    let mut scrolls = ScrollTable::default();
    scrolls.apply_ops(&mut vec![
        ScrollOp::Add { front: ScrollFront { viewport, tracker, hover, grab: ControlId::NONE },
            thumb: None, reveal: Reveal::Never, observe: false },
        ScrollOp::Thumb { viewport, geom: thumb_geom(120.0, 1120.0) },
    ]);
    let id = rows[0].id;
    let point = Point { x: 70.0, y: 110.0 };
    let moved = |at, decided| Report::Dragged { target: id, contact: 1,
        update: DragUpdate { phase: Phase::Free, from: Point::default(), at, delta: at, decided } };
    let mut out = Vec::with_capacity(16);
    rig.tick(&[press(id), moved(point, true)], &mut out)?;
    assert_eq!(rig.scene.hits().visible_rect(id), Some([70.0, 110.0, 150.0, 150.0]));
    assert_eq!(rig.scene.hit(Point { x: 140.0, y: 140.0 }, windows_scene::ContactKind::Mouse).unwrap().id, id);
    let mut front = Front { scene: &mut rig.scene, back: &rig.back, env: rig.env };
    scrolls.reorder_scroll(rig.controls.reorder_pointer(), &mut front)?;
    let animations = front.scene.census().animations;
    let allocations = crate::counting::allocations();
    for _ in 0..100 { scrolls.reorder_scroll(rig.controls.reorder_pointer(), &mut front)?; }
    assert_eq!(crate::counting::allocations(), allocations);
    assert_eq!(front.scene.census().animations, animations);
    rig.back.request_commit()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut events = Vec::new();
    while rig.scene.hits().offset(viewport).y < 363.5 {
        assert!(std::time::Instant::now() < deadline, "tracker did not reach the last row");
        windows_window::pump();
        events.clear();
        rig.scene.drain_events(&mut events);
        let mut front = Front { scene: &mut rig.scene, back: &rig.back, env: rig.env };
        rig.controls.refresh_reorder(&mut front, &mut out)?;
        scrolls.reorder_scroll(rig.controls.reorder_pointer(), &mut front)?;
        assert!(front.scene.hits().shifted(1).into_iter().zip([70.0, 110.0, 150.0, 150.0])
            .all(|(actual, expected)| (actual - expected).abs() < 0.0001));
        assert_eq!(front.scene.hit(Point { x: 140.0, y: 140.0 }, windows_scene::ContactKind::Mouse).unwrap().id, id);
        rig.back.request_commit()?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(rig.scene.hits().offset(viewport).y <= 364.001);
    assert!(matches!(out.last().map(|i| i.what), Some(What::Reordered(ReorderUpdate { to: 7, .. }))));
    rig.tick(&[Report::Released { target: id, contact: 1, at: point }], &mut out)?;
    scrolls.reorder_scroll(rig.controls.reorder_pointer(),
        &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    assert!(matches!(out.last().map(|i| i.what), Some(What::ReorderEnded(Some(ReorderUpdate { to: 7, .. })))));
    let epoch = rig.controls.take_preview_release().unwrap();
    rig.controls.finish_reorder(epoch, &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    rig.scene.finish_drag_preview(epoch);
    assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));

    let target = rows[7].id;
    rig.tick(&[press(target), Report::Dragged { target, contact: 2, update: DragUpdate {
        phase: Phase::Free, from: Point { x: 70.0, y: 90.0 }, at: Point { x: 70.0, y: 10.0 },
        delta: Point { x: 0.0, y: -80.0 }, decided: true,
    } }], &mut out)?;
    scrolls.reorder_scroll(rig.controls.reorder_pointer(),
        &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    rig.back.request_commit()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while rig.scene.hits().offset(viewport).y > 300.0 {
        assert!(std::time::Instant::now() < deadline, "top edge did not reverse scrolling");
        windows_window::pump();
        events.clear();
        rig.scene.drain_events(&mut events);
        rig.controls.refresh_reorder(&mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env }, &mut out)?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    rig.tick(&[Report::Canceled { target, contact: 2 }], &mut out)?;
    let stopped = rig.scene.hits().offset(viewport).y;
    scrolls.reorder_scroll(rig.controls.reorder_pointer(),
        &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    rig.back.request_commit()?;
    let settle = std::time::Instant::now() + std::time::Duration::from_millis(250);
    while std::time::Instant::now() < settle {
        windows_window::pump();
        events.clear();
        rig.scene.drain_events(&mut events);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!((rig.scene.hits().offset(viewport).y - stopped).abs() < 0.001);
    assert_eq!(out.last().map(|i| i.what), Some(What::ReorderEnded(None)));
    assert!(rows.iter().all(|r| r.state.get() == Vector2::zero()));
    let census = *rig.scene.census();
    for _ in 0..100 {
        scrolls.reorder_scroll(None, &mut Front { scene: &mut rig.scene, back: &rig.back, env: rig.env })?;
    }
    assert_eq!(*rig.scene.census(), census);
    assert!(rig.scene.begin_drag_preview(rows[0].node, rows[0].id, &rig.back));
    rig.scene.show_drag_placeholder(0.25, &rig.back);
    drop(rig);
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
