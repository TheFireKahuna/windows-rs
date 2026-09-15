use super::*;
use crate::{
    build::{Host, mount},
    input::{KeyEvent, KeyKind, Mods, PointerFlags, PointerType, Sample},
    signal::{Cell, Owner},
    widget::{ScalarPart, ScalarValue},
};
use windows_scene::{Model, Point};

fn publish(
    down: &mut crate::seam::Down,
    controls: &mut Controls,
    front: &mut Front<'_>,
) -> Result<()> {
    down.clear();
    crate::signal::flush();
    Host::flush(&mut down.patch);
    Host::with(|h| {
        h.fill(down);
    });
    front.scene.apply(&mut down.patch, front.back, front.env)?;
    controls.adopt(&down.chrome, &down.released, front)
}

fn press(target: ControlId) -> Report {
    Report::Pressed {
        target,
        contact: 1,
        buttons: 1,
        sample: Sample {
            id: 1,
            ptype: PointerType::Mouse,
            flags: PointerFlags(0),
            at: Point::default(),
            raw: Point::default(),
            contact: (0.0, 0.0),
            pen: None,
            time: 0,
            qpc: 0,
        },
    }
}

fn drag(target: ControlId, fraction: f32) -> Report {
    Report::Dragged {
        target,
        contact: 1,
        update: crate::gesture::DragUpdate {
            phase: crate::gesture::DragPhase::Locked(crate::gesture::Axis::Vertical),
            delta: Point {
                x: 0.0,
                y: -fraction * crate::widget::TURN_SPAN,
            },
            from: Point::default(),
            at: Point::default(),
            decided: true,
        },
    }
}

#[test]
fn native_scalar_parts_keep_one_writer_and_reject_stale_commits() -> Result<()> {
    windows_window::ensure_dispatcher_queue(windows_window::Apartment::Asta)?;
    let (_owner, result) =
        Owner::scope(|| -> Result<()> {
            let _ = crate::build::tests::fixture();
            let (env, scope) = Host::with(|h| (h.env, h.root_scope));
            let mut model = Model::new(crate::layout::root());
            model.set_window(windows_numerics::Vector2 { x: 800.0, y: 600.0 });
            Host::install(model, env, scope);
            let window = windows_window::Window::new("scalar ownership")
                .size_dips(800.0, 600.0)
                .create()?;
            let back = Backends::new(
                windows_composition::Compositor::new()?,
                &windows_d2d::Gpu::for_window()?,
                windows_text::FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]),
            )?;
            Host::install_text(back.ladder().clone())?;
            let mut scene = Scene::new_at(
                window.handle(),
                &back,
                env,
                windows_scene::BackdropSpec::default(),
            )?;
            let mut front = Front {
                scene: &mut scene,
                back: &back,
                env,
            };
            let source = Cell::new(ScalarValue {
                value: 0.25,
                epoch: 0,
            });
            let accepted = Cell::new(0_usize);
            let geom = crate::build::geometry(&[
                windows_scene::PathVerb::Move {
                    to: windows_numerics::Vector2::default(),
                    filled: false,
                },
                windows_scene::PathVerb::Line(windows_numerics::Vector2 { x: 30.0, y: 30.0 }),
                windows_scene::PathVerb::End { closed: false },
            ]);
            let root = Host::with(|h| h.model().root());
            let _held = mount(
                crate::layout::grid(())
                    .at(
                        0,
                        0,
                        crate::layout::grid(())
                            .scalar_part(ScalarPart::Rotation { from: 0.0, to: 4.0 }),
                    )
                    .at(
                        0,
                        0,
                        crate::widget::path(geom)
                            .stroke(crate::role::DataRole(1), crate::role::Metric::HairlineW)
                            .scalar_part(ScalarPart::TrimEnd),
                    )
                    .width(crate::role::Metric::CardMinW)
                    .height(crate::role::Metric::CardMinW)
                    .turn_source(source, crate::widget::Range::UNIT)
                    .on_commit(move |value| {
                        accepted.set(accepted.get() + 1);
                        source.set(ScalarValue {
                            value,
                            epoch: source.get().epoch,
                        });
                    }),
                root,
            );
            let mut controls = Controls::new();
            let mut down = crate::seam::Down::default();
            publish(&mut down, &mut controls, &mut front)?;
            let row = *down
                .chrome
                .iter()
                .find(|r| r.scalar_parts.iter().flatten().count() == 2)
                .unwrap();
            let id = row.id;
            let mut out = Vec::with_capacity(32);
            let before = *front.scene.census();
            controls.tick(&[press(id), drag(id, 0.25)], &mut front, &mut out)?;
            assert_eq!(source.get().value, 0.25, "app work is deliberately delayed");
            assert_eq!(controls.rows.get(id).unwrap().fraction, 0.5);
            assert!(front.scene.census().animations >= before.animations + 2);
            assert_eq!(front.scene.census().visuals_minted, before.visuals_minted);
            // A resize may move the same nodes, but cannot adopt the older source fraction.
            Host::with(|h| {
                h.model()
                    .set_window(windows_numerics::Vector2 { x: 900.0, y: 650.0 })
            });
            publish(&mut down, &mut controls, &mut front)?;
            assert_eq!(controls.rows.get(id).unwrap().fraction, 0.5);
            controls.tick(
                &[Report::Released {
                    target: id,
                    contact: 1,
                    at: Point::default(),
                }],
                &mut front,
                &mut out,
            )?;
            // A second gesture starts before the application receives the first commit.
            controls.tick(&[press(id), drag(id, 0.25)], &mut front, &mut Vec::new())?;
            Host::with(|h| h.dispatch(&out));
            assert_eq!(accepted.get(), 1);
            publish(&mut down, &mut controls, &mut front)?;
            assert_eq!(controls.pressed, Some(id));
            assert_eq!(
                controls.rows.get(id).unwrap().fraction,
                0.75,
                "a commit echo must preserve the newer gesture"
            );
            // Same-value document replacement still invalidates a queued commit by epoch.
            out.clear();
            controls.tick(
                &[Report::Released {
                    target: id,
                    contact: 1,
                    at: Point::default(),
                }],
                &mut front,
                &mut out,
            )?;
            source.set(ScalarValue {
                value: 0.5,
                epoch: 1,
            });
            publish(&mut down, &mut controls, &mut front)?;
            Host::with(|h| h.dispatch(&out));
            assert_eq!(accepted.get(), 1);
            assert_eq!(source.get().value, 0.5);
            // Cancellation restores the starting value and emits no accepted commit.
            out.clear();
            controls.tick(
                &[
                    press(id),
                    drag(id, 0.2),
                    Report::Canceled {
                        target: id,
                        contact: 1,
                    },
                ],
                &mut front,
                &mut out,
            )?;
            assert_eq!(controls.rows.get(id).unwrap().fraction, 0.5);
            assert!(matches!(out.last().unwrap().what, What::Canceled(_)));
            Host::with(|h| h.dispatch(&out));
            assert_eq!(accepted.get(), 1);
            // UIA, keyboard and rotary share snapping and produce one committed action each.
            out.clear();
            controls.automation(
                &[crate::uia::Action::SetValue(id, 2.0)],
                &mut front,
                &mut out,
            )?;
            controls.tick(
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
                        steps: 1.0,
                        degrees: 1.0,
                    },
                ],
                &mut front,
                &mut out,
            )?;
            assert_eq!(
                out.iter()
                    .filter(|i| matches!(i.what, What::Scalar { commit: true, .. }))
                    .count(),
                3
            );
            assert!(out.iter().all(
                |i| matches!(i.what, What::Scalar { value, .. } if (0.0..=1.0).contains(&value))
            ));
            // The stock switch uses the same scene-owned offset path.
            let on = Cell::new(false);
            let _toggle = mount(crate::widget::toggle(on), root);
            publish(&mut down, &mut controls, &mut front)?;
            let before = front.scene.census().animations;
            on.set(true);
            publish(&mut down, &mut controls, &mut front)?;
            assert!(
                front.scene.census().animations > before,
                "source adoption springs the real composition thumb"
            );
            // Measure the production driver and compositor calls after their buffers warm.
            // This counts Rust allocations on this thread, not allocations inside Windows.
            out.clear();
            controls.tick(&[press(id), drag(id, 0.1)], &mut front, &mut out)?;
            let animations = front.scene.census().animations;
            let before = crate::counting::allocations();
            for step in 0..1_000 {
                out.clear();
                controls.tick(
                    &[drag(id, if step % 2 == 0 { 0.2 } else { 0.1 })],
                    &mut front,
                    &mut out,
                )?;
            }
            let allocated = crate::counting::allocations() - before;
            assert_eq!(
                front.scene.census().animations - animations,
                2_000,
                "the allocation probe must actually retarget both native parts"
            );
            assert_eq!(allocated, 0, "warm scalar retargeting allocated");
            out.clear();
            controls.tick(
                &[Report::Canceled {
                    target: id,
                    contact: 1,
                }],
                &mut front,
                &mut out,
            )?;
            publish(&mut down, &mut controls, &mut front)?;
            let before = crate::counting::allocations();
            for _ in 0..100 {
                publish(&mut down, &mut controls, &mut front)?;
            }
            assert_eq!(
                crate::counting::allocations() - before,
                0,
                "unchanged flush/apply allocated"
            );
            Ok(())
        });
    result
}
