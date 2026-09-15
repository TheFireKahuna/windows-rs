use super::scalar_tests::{press, publish};
use super::*;
use crate::{
    build::Host,
    signal::{Cell, Owner},
    widget::button,
};
use windows_scene::{Model, Point};

fn hover(from: Option<ControlId>, to: Option<ControlId>) -> Report {
    Report::HoverChanged {
        from,
        to,
        at: Point::default(),
        qpc: 0,
    }
}

#[test]
fn native_reveal_preserves_controls_and_retires_targets() -> Result<()> {
    windows_window::ensure_dispatcher_queue(windows_window::Apartment::Asta)?;
    let (_owner, result) = Owner::scope(|| -> Result<()> {
        let _ = crate::build::tests::fixture();
        let (env, scope) = Host::with(|h| (h.env, h.root_scope));
        let mut model = Model::new(crate::layout::root());
        model.set_window(windows_numerics::Vector2::new(800.0, 600.0));
        Host::install(model, env, scope);
        let window = windows_window::Window::new("interaction reveal")
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
        let shown = Cell::new(true);
        let held = crate::build::Ui::mount_root(|ui| {
            ui.node(crate::layout::Preset::Row)
                .interaction_scope()
                .row(|ui| {
                    button(ui, "Route").key("route");
                    ui.when(shown, |ui| {
                        button(ui, "Remove").key("remove").reveal_on_interaction();
                    });
                });
        });
        let mut controls = Controls::new();
        let mut down = crate::seam::Down::default();
        publish(&mut down, &mut controls, &mut front)?;
        let named = |name| {
            Host::with(|h| {
                h.controls
                    .iter()
                    .find(|(_, r)| r.key == Some(name))
                    .unwrap()
                    .0
            })
        };
        let route = named("route");
        let remove = named("remove");
        let scope = controls.rows.get(route).unwrap().hover_scope.unwrap();
        let reveal = controls.rows.get(scope).unwrap().reveal;
        assert!(!controls.rows.get(scope).unwrap().observes_hover);
        assert!(
            !front
                .scene
                .hits()
                .entry(scope)
                .unwrap()
                .flags
                .contains(windows_scene::HitFlags::INTERACTIVE)
        );
        assert!(
            front
                .scene
                .hits()
                .entry(remove)
                .unwrap()
                .flags
                .contains(windows_scene::HitFlags::INTERACTIVE)
        );
        let mut out = Vec::with_capacity(16);
        let before = *front.scene.census();
        controls.tick(&[hover(None, Some(route))], &mut front, &mut out)?;
        assert!(controls.revealed.contains(&(scope, reveal)));
        assert!(
            out.is_empty(),
            "cosmetic hover must not wake the application"
        );
        let animations = front.scene.census().animations;
        for _ in 0..16 {
            controls.tick(
                &[
                    hover(Some(route), Some(remove)),
                    hover(Some(remove), Some(route)),
                ],
                &mut front,
                &mut out,
            )?;
            controls.tick(&[], &mut front, &mut out)?;
        }
        // Button washes may move; the reveal target retains the same ownership and node.
        assert_eq!(
            front.scene.census().animations - animations,
            64,
            "crossing children must animate only their existing washes"
        );
        assert!(controls.revealed.contains(&(scope, reveal)));
        assert_eq!(front.scene.census().visuals_minted, before.visuals_minted);
        assert!(out.is_empty());
        controls.tick(
            &[
                Report::FocusChanged {
                    from: None,
                    to: Some(remove),
                },
                hover(Some(route), None),
            ],
            &mut front,
            &mut out,
        )?;
        assert!(
            controls.revealed.contains(&(scope, reveal)),
            "keyboard focus must keep actions visible"
        );
        controls.tick(
            &[Report::FocusChanged {
                from: Some(remove),
                to: None,
            }],
            &mut front,
            &mut out,
        )?;
        assert!(controls.revealed.iter().all(|(_, node)| node.is_none()));
        assert!(front.scene.census().animations > animations);
        controls.tick(&[press(remove)], &mut front, &mut out)?;
        assert!(
            controls.revealed.contains(&(scope, reveal)),
            "touch press must reveal without hover"
        );
        controls.tick(
            &[Report::Canceled {
                target: remove,
                contact: 1,
            }],
            &mut front,
            &mut out,
        )?;
        assert!(controls.revealed.iter().all(|(_, node)| node.is_none()));
        out.clear();
        controls.tick(&[hover(None, Some(route))], &mut front, &mut out)?;
        for show in [false, true, false, true] {
            shown.set(show);
            publish(&mut down, &mut controls, &mut front)?;
            let target = controls.rows.get(scope).unwrap().reveal;
            assert_eq!(target.is_none(), !show);
            assert_eq!(
                controls.revealed.iter().any(|(_, node)| !node.is_none()),
                show
            );
        }
        assert!(
            controls.rows.get(remove).is_none(),
            "retired control generation must stay absent"
        );
        assert_eq!(front.scene.census().visuals_live, before.visuals_live);
        let settled = *front.scene.census();
        for _ in 0..16 {
            controls.tick(&[], &mut front, &mut out)?;
        }
        assert_eq!(*front.scene.census(), settled);
        drop(held);
        publish(&mut down, &mut controls, &mut front)?;
        assert!(controls.revealed.iter().all(|(_, node)| node.is_none()));
        controls.tick(&[hover(Some(route), Some(remove))], &mut front, &mut out)?;
        assert!(out.is_empty());
        Ok(())
    });
    result
}
