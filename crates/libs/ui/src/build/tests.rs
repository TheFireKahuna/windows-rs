//! Tests for the build lowering, driven headless.
//!
//! `Model` owns no COM, so a whole mount runs with no window, no device and no compositor,
//! and the ops it emits are read back off the patch.

use super::arena::{Build, MaskSeed, Part};
use super::*;
use crate::layout::{Len, stack};
use crate::role::{
    AccentId, Density, Elevation, Fill, Metric, Polarity, Role, Scope, Stroke, Text, TypeRole,
};
use crate::widget::{Flow, Motion, StatePolicy, Wash};
use windows_color::{DisplayCapability, OutputTransform, Radiance};
use windows_numerics::Vector2;
use windows_scene::{Env, Model, Op, Paint, SinkPatch, taffy};
use windows_text::FontLadder;

/// Creates this thread's text engine and host with an immutable palette reference.
pub(crate) fn fixture() -> SinkPatch {
    fixture_at(96.0)
}

#[test]
fn independent_hosts_retheme_existing_recipes_and_preserve_state() {
    let jobs: Vec<_> = [Polarity::Dark, Polarity::Light]
        .into_iter()
        .map(|polarity| {
            std::thread::spawn(move || {
                let mut patch = fixture();
                let root_scope = Host::with(|h| h.root_scope);
                let (_owner, held) = crate::signal::Owner::scope(|| {
                    let selected = crate::signal::Cell::new(true);
                    let disabled = crate::signal::Cell::new(true);
                    const ROWS: &[crate::widget::RoleSet] = &[crate::widget::RoleSet {
                        fill: Some(Fill::Surface),
                        stroke: Some(Stroke::Subtle),
                        text: Text::Secondary,
                    }];
                    const CHROME: crate::widget::Chrome = crate::widget::Chrome {
                        selected: Some(crate::widget::RoleSet {
                            fill: Some(Fill::AccentSubtle),
                            stroke: Some(Stroke::Accent),
                            text: Text::Primary,
                        }),
                        disabled: Some(crate::widget::RoleSet {
                            fill: None,
                            stroke: Some(Stroke::Subtle),
                            text: Text::Disabled,
                        }),
                        ..crate::widget::Chrome::new(ROWS[0], Metric::Radius)
                    };
                    let held = mount(
                        stack((
                            crate::widget::button_with(
                                "Recipe",
                                crate::widget::TextStyle::new(TypeRole::Body),
                            )
                            .appearance(CHROME)
                            .selected(selected)
                            .disabled(disabled),
                            crate::widget::field("draft"),
                        )),
                        root(),
                    );
                    flush(&mut patch);
                    let id = Host::with(|h| {
                        h.controls
                            .iter()
                            .find(|(_, c)| c.chrome == Some(CHROME))
                            .unwrap()
                            .0
                    });
                    assert_eq!(
                        Host::with(|h| h.control(id).unwrap().state),
                        crate::widget::ModelState::Disabled
                    );
                    selected.set(false);
                    crate::signal::flush();
                    assert_eq!(
                        Host::with(|h| h.control(id).unwrap().state),
                        crate::widget::ModelState::Disabled
                    );
                    selected.set(true);
                    disabled.set(false);
                    crate::signal::flush();
                    assert_eq!(
                        Host::with(|h| h.control(id).unwrap().state),
                        crate::widget::ModelState::Selected
                    );
                    let counts = Host::with(|h| {
                        (
                            h.appearances.iter().count(),
                            h.controls.iter().count(),
                            h.fields.iter().count(),
                        )
                    });
                    for i in 0..12 {
                        let theme = Scope {
                            polarity: if i % 2 == 0 { polarity } else { Polarity::Dark },
                            density: if i % 2 == 0 {
                                Density::Compact
                            } else {
                                Density::Comfortable
                            },
                            ..root_scope
                        };
                        patch.clear();
                        Host::with(|h| h.set_theme(theme, windows_scene::BackdropSpec::default()));
                        flush(&mut patch);
                        assert_eq!(filled().theme.unwrap().0, theme);
                        assert!(
                            !patch
                                .ops()
                                .iter()
                                .any(|op| matches!(op, Op::New { .. } | Op::Drop { .. })),
                            "theme changes retain control identity"
                        );
                        Host::with(|h| {
                            assert_eq!(h.root_scope, theme);
                            assert_eq!(
                                h.control(id).unwrap().state,
                                crate::widget::ModelState::Selected
                            );
                            assert_eq!(h.control(id).unwrap().scope.polarity, theme.polarity);
                            assert_eq!(
                                (
                                    h.appearances.iter().count(),
                                    h.controls.iter().count(),
                                    h.fields.iter().count()
                                ),
                                counts
                            );
                        });
                        patch.clear();
                        Host::with(|h| h.set_theme(theme, windows_scene::BackdropSpec::default()));
                        flush(&mut patch);
                        assert!(patch.ops().is_empty(), "an unchanged theme parks");
                        assert!(filled().theme.is_none(), "an equal transaction sends no wake");
                    }
                    held
                });
                drop(held);
                Host::with(|h| {
                    assert_eq!(h.appearances.iter().count(), 0);
                    assert_eq!(h.controls.iter().count(), 0);
                    assert_eq!(h.fields.iter().count(), 0);
                });
            })
        })
        .collect();
    for job in jobs {
        job.join().unwrap();
    }
}

#[test]
fn choice_dispatch_preserves_canonical_selection_when_an_edit_is_declined() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let selected = crate::signal::Cell::new(0_u8);
        let attempts = crate::signal::Cell::new(0_u8);
        let _held = mount(crate::widget::segmented((move || selected.get(), move |next| {
            attempts.set(attempts.get() + 1);
            if next != 2 { selected.set(next); }
        }), &[("First", 0), ("Second", 1), ("Unavailable", 2)]), root());
        crate::signal::flush();
        flush(&mut patch);
        let ids = Host::with(|h| h.controls.iter().filter(|(_, c)| c.uia == crate::widget::UiaRole::RadioButton).map(|(id, _)| id).collect::<Vec<_>>());
        for (at, expected) in [(1, 1), (2, 1)] {
            Host::with(|h| h.dispatch(&[crate::widget::Intent { target: ids[at], what: crate::widget::What::Tapped }]));
            crate::signal::flush();
            assert_eq!(selected.get(), expected);
            Host::with(|h| assert_eq!(h.control(ids[1]).unwrap().state, crate::widget::ModelState::Selected));
        }
        assert_eq!(attempts.get(), 2);
        selected.set(0);
        crate::signal::flush();
        Host::with(|h| assert_eq!(h.control(ids[0]).unwrap().state, crate::widget::ModelState::Selected));
    });
}

#[test]
fn a_gradient_fill_keeps_its_geometry_mask_and_resources_on_edit() {
    let (_resource_owner, ()) = crate::signal::Owner::scope(|| {
        use windows_scene::{Mask, PathVerb, ResOp, Spread};
        let mut patch = fixture();
        let hue = crate::role::DataRole(1);
        let verts = [
            PathVerb::Move {
                to: Vector2 { x: 0.0, y: 0.0 },
                filled: true,
            },
            PathVerb::Line(Vector2 { x: 80.0, y: 40.0 }),
            PathVerb::Line(Vector2 { x: 0.0, y: 40.0 }),
            PathVerb::End { closed: true },
        ];
        let geom = geometry(&verts);
        let stops = [
            Stop {
                at: 0.0,
                role: hue,
                strength: 0.1,
            },
            Stop {
                at: 1.0,
                role: hue,
                strength: 0.0,
            },
        ];
        let fade = ramp(&stops, Spread::Vertical);
        let _mount = mount(
            crate::widget::path(geom)
                .fill_ramp(fade)
                .width(Metric::RowH)
                .height(Metric::RowH),
            root(),
        );
        flush(&mut patch);
        assert!(patch.ops().iter().any(|op| matches!(op, Op::Mask { mask: Mask::Shape { geom: id, stroke: None }, .. } if *id == geom)));
        assert!(
            patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::Paint { paint: Paint::Ramp(id), .. } if *id == fade))
        );
        patch.clear();
        set_ramp(
            fade,
            &[
                Stop {
                    at: 0.25,
                    ..stops[0]
                },
                stops[1],
            ],
            Spread::Vertical,
        );
        flush(&mut patch);
        assert!(patch.ops().iter().any(|op| matches!(
            op,
            Op::Res {
                op: ResOp::Ramp { .. },
                ..
            }
        )));
        assert!(!patch.ops().iter().any(|op| matches!(op, Op::New { .. })));
        patch.clear();
        flush(&mut patch);
        assert!(
            patch.ops().is_empty(),
            "an unchanged gradient has no idle work"
        );
    });
}

#[test]
fn instrument_edit_retargets_trim_rotation_and_ink_halo_without_reminting() {
    let (_resource_owner, ()) = crate::signal::Owner::scope(|| {
        use crate::role::DataRole;
        use windows_scene::{Anim, Bind, PathVerb, Prop, Spread, Value};
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.25_f32);
        let glow = crate::signal::Cell::new(Role::Data(DataRole(0xfffe)));
        let geometry = geometry(&[
            PathVerb::Move {
                to: Vector2 { x: 0.0, y: 0.0 },
                filled: false,
            },
            PathVerb::Line(Vector2 { x: 20.0, y: 20.0 }),
            PathVerb::End { closed: false },
        ]);
        let gradient = ramp(
            &[
                Stop {
                    at: 0.0,
                    role: DataRole(1),
                    strength: 1.0,
                },
                Stop {
                    at: 1.0,
                    role: DataRole(2),
                    strength: 1.0,
                },
            ],
            Spread::Conic {
                center: [0.5, 0.56],
                start: 0.0,
            },
        );
        let (owner, held) = crate::signal::Owner::scope(|| {
            mount(
                crate::widget::path(geometry)
                    .stroke_ramp(gradient, Metric::HairlineW)
                    .width(crate::role::tests::EXTENT)
                    .height(crate::role::tests::EXTENT)
                    .pivot(Vector2 { x: 64.0, y: 71.68 })
                    .trim(move || value.get())
                    .rotation(move || value.get() * 4.0)
                    .halo(glow),
                root(),
            )
        });
        flush(&mut patch);
        assert!(patch.ops().iter().any(|op| matches!(op,
        Op::Halo { halo: Some(halo), .. } if halo.blur == 9.0)));
        patch.clear();
        value.set(0.75);
        glow.set(Role::Data(DataRole(0xffff)));
        crate::signal::flush();
        flush(&mut patch);
        for (property, target) in [(Prop::TrimEnd, 0.75), (Prop::RotationAngle, 3.0)] {
            assert!(patch.ops().iter().any(|op| matches!(op,
            Op::Bind { prop, bind: Bind::Animate(Anim::Spring { to: Value::Scalar(v), .. }), .. }
            if *prop == property && *v == target)));
        }
        assert!(
            patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::Halo { halo: Some(_), .. }))
        );
        assert!(
            !patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::New { .. } | Op::Res { .. }))
        );
        patch.clear();
        flush(&mut patch);
        assert!(patch.ops().is_empty());
        drop(owner);
        drop(held);
        flush(&mut patch);
        patch.clear();
        value.set(0.1);
        glow.set(Role::Data(DataRole(0xfffe)));
        crate::signal::flush();
        flush(&mut patch);
        assert!(
            patch.ops().is_empty(),
            "unmounted instrument effects must be disposed"
        );
    });
}

/// [`fixture`] at a stated DPI.
///
/// The raster caches are cut in physical pixels, so a mask that is exact at one scale can be
/// degenerate at another. A test that only ever runs at 96 cannot see it.
pub(crate) fn fixture_at(dpi: f32) -> SinkPatch {
    if !super::text::installed() {
        // The real engine, over the two inbox faces the palette names, so every width
        // asserted below is DirectWrite's own advance rather than an invented one.
        super::text::install(FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]))
            .expect("DirectWrite is available on the platform floor");
    }
    // The driver's own root, so a mount here is arranged exactly as a window arranges it. A
    // root written here instead can differ — a flex row gives a mounted child its content
    // width, which leaves a scroll viewport zero DIPs wide and hit-testing nothing.
    let mut model = Model::new(crate::layout::root());
    model.set_window(Vector2 { x: 800.0, y: 600.0 });
    Host::install(
        model,
        Env::new(
            dpi,
            OutputTransform::for_display(DisplayCapability::Sdr, 1000.0),
        ),
        Scope::root(
            crate::role::tests::palette(),
            AccentId(0),
            Density::Comfortable,
        ),
    );
    // The root's own `New` op rides the first flush. Draining it leaves the patch carrying
    // only what the test itself mounts.
    let mut patch = SinkPatch::new();
    Host::with(|h| h.flush(&mut patch));
    patch.clear();
    patch
}

fn flush(patch: &mut SinkPatch) {
    Host::with(|h| h.flush(patch));
}

fn root() -> windows_scene::GroupId {
    Host::with(|h| h.model().root())
}

#[test]
#[cfg(feature = "test-support")]
fn hover_scope_reaches_dynamic_children_and_clears_on_unmount() {
    use crate::signal::{Cell, Owner};
    use crate::widget::{Intent, What, button};
    let mut patch = fixture();
    let (_owner, (hovered, added)) = Owner::scope(|| (Cell::new(false), Cell::new(false)));
    let (content, mounted) = Owner::scope(|| {
        mount(
            stack((
                button("First").key("first"),
                switch(
                    move || added.get(),
                    |added| {
                        button(if *added { "Added" } else { "Initial" })
                            .key("dynamic")
                            .erase()
                    },
                ),
            ))
            .hover_scope(hovered),
            root(),
        )
    });
    let mut driver = crate::driver::testing::LayoutDriver::default();
    for _ in 0..8 {
        patch.clear();
        driver.flush(&mut patch);
    }
    let observer = Host::with(|h| {
        h.controls
            .iter()
            .find_map(|(id, row)| row.hovered.map(|_| id))
    })
    .unwrap();
    for change in [false, true] {
        added.set(change);
        for _ in 0..8 {
            patch.clear();
            driver.flush(&mut patch);
        }
        Host::with(|h| {
            for name in ["first", "dynamic"] {
                let row = h
                    .controls
                    .iter()
                    .find(|(_, row)| row.key == Some(name))
                    .unwrap()
                    .1;
                assert_eq!(row.front.hover_scope, Some(observer));
            }
            h.dispatch(&[Intent {
                target: observer,
                what: What::Hovered(true),
            }]);
        });
        assert!(hovered.get());
    }
    drop(content);
    drop(mounted);
    assert!(!hovered.get(), "a surviving observer cell must be cleared");
    Host::with(|h| {
        h.dispatch(&[Intent {
            target: observer,
            what: What::Hovered(true),
        }])
    });
    assert!(
        !hovered.get(),
        "an event queued before unmount must be ignored"
    );
}

/// Returns everything the host has produced since the last call, as one batch.
fn filled() -> crate::seam::Down {
    let mut down = crate::seam::Down::default();
    Host::with(|h| h.fill(&mut down));
    down
}

/// Returns the pick table the regions declared so far, as the tick builds it.
fn picks() -> crate::present::Picks {
    let mut picks = crate::present::Picks::default();
    picks.apply(&filled().regions);
    picks
}

/// Returns a bare rounded box, the smallest view that mints a sprite.
fn plate() -> View {
    El::<Any>::seed(crate::layout::Preset::Bare).sprite(
        MaskSeed::Box {
            radius: Some(Len::Metric(Metric::Radius)),
        },
        Role::Fill(Fill::Surface),
        Part::Fill,
    )
}

#[test]
fn vertical_labels_keep_their_rotated_extent_after_publish_and_updates() {
    for dpi in [96.0, 144.0, 192.0] {
        let mut patch = fixture_at(dpi);
        let word = crate::signal::Cell::new("Inspector");
        let held = mount(
            stack((
                crate::widget::vertical_label(crate::widget::shown(move || word.get())),
                crate::widget::label(crate::widget::shown(move || word.get())),
            ))
            .align(crate::layout::Align::Start),
            root(),
        );
        for (value, width) in [
            ("Inspector", 600.0),
            ("Longer inspector label", 900.0),
            ("", 320.0),
            ("I", 700.0),
        ] {
            word.set(value);
            Host::with(|h| h.model().set_window(Vector2 { x: width, y: 600.0 }));
            crate::signal::flush();
            flush(&mut patch);
            let sizes = || {
                Host::with(|h| {
                    let nodes: Vec<_> = h
                        .mounts
                        .iter()
                        .filter(|(_, m)| m.text.is_some())
                        .map(|(_, m)| m.node)
                        .collect();
                    let key = h.mounts.iter().filter_map(|(_, m)| m.text).nth(1).unwrap();
                    let intrinsic = text::measure(windows_scene::MeasureIn {
                        key,
                        class: h.model().solved(nodes[1]).class,
                        known: (None, None),
                        available: (
                            windows_scene::Avail::MaxContent,
                            windows_scene::Avail::MaxContent,
                        ),
                    });
                    (h.model().solved(nodes[0]).size, intrinsic)
                })
            };
            let (v, h) = sizes();
            if !value.is_empty() {
                assert!(v.x > 0.0 && v.y > 0.0);
                assert!((v.y - h.x).abs() < 2.0, "{dpi}: {v:?} vs {h:?}");
                assert!((v.x - h.y).abs() < 2.0, "{dpi}: {v:?} vs {h:?}");
            }
            patch.clear();
            flush(&mut patch);
            assert!(
                patch.ops().is_empty(),
                "settled vertical text must emit nothing"
            );
            assert_eq!(sizes().0, v);
        }
        drop(held);
    }
}

#[test]
fn dynamic_labels_fit_replacement_text_without_previous_width_padding() {
    for dpi in [96.0, 144.0, 192.0] {
        let mut patch = fixture_at(dpi);
        let word = crate::signal::Cell::new("Inspector");
        let held = mount(
            stack(crate::widget::label(crate::widget::shown(move || {
                word.get()
            })))
            .align(crate::layout::Align::Start),
            root(),
        );
        for value in [
            "Inspector",
            "Longer inspector label",
            "",
            "I",
            "Replacement",
        ] {
            word.set(value);
            crate::signal::flush();
            flush(&mut patch);
            Host::with(|h| {
                let (node, key) = h
                    .mounts
                    .iter()
                    .find_map(|(_, m)| m.text.map(|k| (m.node, k)))
                    .unwrap();
                let actual = h.model().solved(node);
                let expected = text::measure(windows_scene::MeasureIn {
                    key,
                    class: actual.class,
                    known: (None, None),
                    available: (
                        windows_scene::Avail::MaxContent,
                        windows_scene::Avail::MaxContent,
                    ),
                });
                assert!(
                    (actual.size.x - expected.x).abs() < 2.0,
                    "{dpi}: {value:?}: {:?} vs {expected:?}",
                    actual.size
                );
            });
            patch.clear();
            flush(&mut patch);
            assert!(patch.ops().is_empty(), "settled label must emit nothing");
        }
        drop(held);
    }
}

// ── lowering ─────────────────────────────────────────────────────────────────────

/// A slot with one sprite and no children lowers to that sprite: one visual, no group.
#[test]
fn a_single_sprite_slot_costs_one_visual() {
    let mut patch = fixture();
    let _mount = mount(plate(), root());
    flush(&mut patch);

    let minted: Vec<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::New { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(minted, vec![windows_scene::NodeKind::Sprite]);
}

/// A container mints a group, and its children land in paint order.
///
/// Child order is z-order, so the ops are asserted as a sequence rather than as a set.
#[test]
fn children_mount_in_paint_order() {
    let mut patch = fixture();
    let _mount = mount(stack((plate(), plate(), plate())), root());
    flush(&mut patch);

    let minted: Vec<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::New { id, after, .. } => Some((*id, *after)),
            _ => None,
        })
        .collect();
    assert_eq!(minted.len(), 4, "one group and three sprites");
    assert_eq!(
        minted[0].1, None,
        "the group sits at the bottom of its parent"
    );
    assert_eq!(
        minted[1].1, None,
        "the first child is the bottom of the group"
    );
    assert_eq!(
        minted[2].1,
        Some(minted[1].0),
        "the second sits above the first"
    );
    assert_eq!(minted[3].1, Some(minted[2].0), "the third above the second");
}

/// A constant channel lowers to one `Set` at mount: no graph node, no effect.
///
/// A static screen therefore costs sprites and nothing else.
#[test]
fn a_constant_channel_produces_no_effect() {
    let mut patch = fixture();
    let _mount = mount(plate().opacity(0.5), root());
    flush(&mut patch);

    // `.opacity` declares `Motion::Chrome`, so a value routed through an effect arrives as a
    // spring. A plain `Set` means the constant path was taken and no effect was created.
    assert!(
        !patch.ops().iter().any(|op| matches!(
            op,
            Op::Bind {
                bind: windows_scene::Bind::Animate(_),
                ..
            }
        )),
        "a constant must not start an animation, and therefore must not have an effect"
    );
    let sets = patch
        .ops()
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Bind {
                    bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(v)),
                    prop: windows_scene::Prop::Opacity,
                    ..
                } if (*v - 0.5).abs() < f32::EPSILON
            )
        })
        .count();
    assert_eq!(sets, 1);
}

/// A reactive channel lowers to one effect, and writing its cell re-binds the property.
#[test]
fn a_reactive_channel_tracks_its_cell() {
    let mut patch = fixture();
    let alpha = crate::signal::Cell::new(0.25_f32);
    let _mount = mount(plate().opacity(alpha), root());
    flush(&mut patch);
    patch.clear();

    alpha.set(0.75);
    crate::signal::flush();
    flush(&mut patch);

    let bound = patch.ops().iter().any(|op| {
        matches!(
            op,
            Op::Bind {
                bind: windows_scene::Bind::Animate(windows_scene::Anim::Spring {
                    to: windows_scene::Value::Scalar(v),
                    ..
                }),
                ..
            } if (*v - 0.75).abs() < f32::EPSILON
        )
    });
    assert!(bound, "a cell write must reach the sink it was bound to");
}

/// A channel's first value is a state, not a transition into one.
///
/// Animating it would sweep every bound property up from whatever the compositor happens to
/// hold — a meter would fill on mount — and a value the compositor never held is one a
/// natural-motion spring cannot reliably start from, which is what left a layer declared
/// invisible unable to come back.
#[test]
fn a_channel_lands_on_its_first_value_and_animates_to_every_later_one() {
    let mut patch = fixture();
    let alpha = crate::signal::Cell::new(0.0_f32);
    let _mount = mount(plate().opacity(alpha), root());
    flush(&mut patch);

    let kinds: Vec<&str> = binds(&patch, windows_scene::Prop::Opacity)
        .iter()
        .map(|op| match op {
            Op::Bind {
                bind: windows_scene::Bind::Set(_),
                ..
            } => "set",
            _ => "animate",
        })
        .collect();
    assert_eq!(kinds, vec!["set"], "the mount lands, it does not fade in");

    patch.clear();
    alpha.set(1.0);
    crate::signal::flush();
    flush(&mut patch);
    let kinds: Vec<&str> = binds(&patch, windows_scene::Prop::Opacity)
        .iter()
        .map(|op| match op {
            Op::Bind {
                bind: windows_scene::Bind::Set(_),
                ..
            } => "set",
            _ => "animate",
        })
        .collect();
    assert_eq!(kinds, vec!["animate"], "and every value after it is a move");
}

/// Editing data used by a channel must not restart unchanged chrome animations.
#[test]
fn unchanged_channel_output_does_not_retarget_and_keeps_tracking() {
    let mut patch = fixture();
    let state = crate::signal::Cell::new((true, 0_u32));
    let alternate = crate::signal::Cell::new(1.0_f32);
    let _mount = mount(
        plate().opacity(move || {
            let (enabled, _) = state.get();
            if enabled { 1.0 } else { alternate.get() }
        }),
        root(),
    );
    flush(&mut patch);

    // An edit changes the source but leaves the card enabled. Switching sources
    // at the same opacity must also track the newly read dependency.
    for next in [(true, 1), (true, 2), (false, 2)] {
        patch.clear();
        state.set(next);
        crate::signal::flush();
        flush(&mut patch);
        assert!(binds(&patch, windows_scene::Prop::Opacity).is_empty());
    }

    patch.clear();
    alternate.set(0.45);
    crate::signal::flush();
    flush(&mut patch);
    assert!(matches!(
        binds(&patch, windows_scene::Prop::Opacity).as_slice(),
        [Op::Bind {
            bind: windows_scene::Bind::Animate(windows_scene::Anim::Spring {
                to: windows_scene::Value::Scalar(0.45),
                ..
            }),
            ..
        }]
    ));
}

/// A channel that fades to zero comes back.
///
/// The tier crossfade in a channel graph is exactly this shape — one layer's opacity to zero
/// while another's comes up — and a value that cannot return leaves the layer dark for the
/// rest of the session.
#[test]
fn an_opacity_that_reaches_zero_binds_again_on_the_way_back() {
    let mut patch = fixture();
    let alpha = crate::signal::Cell::new(1.0_f32);
    let _mount = mount(plate().opacity(alpha), root());
    flush(&mut patch);

    let to = |patch: &SinkPatch| -> Vec<f32> {
        patch
            .ops()
            .iter()
            .filter_map(|op| match op {
                Op::Bind {
                    prop: windows_scene::Prop::Opacity,
                    bind:
                        windows_scene::Bind::Animate(windows_scene::Anim::Spring {
                            to: windows_scene::Value::Scalar(v),
                            ..
                        }),
                    ..
                } => Some(*v),
                _ => None,
            })
            .collect()
    };

    patch.clear();
    alpha.set(0.0);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(to(&patch), vec![0.0], "the fade out is bound");

    patch.clear();
    alpha.set(1.0);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(to(&patch), vec![1.0], "and so is the fade back in");
}

/// An interactive control mints exactly one extra visual, and parks it at zero opacity.
///
/// The wash crossfades compositor-side, so hover costs one visual and no app-thread work.
#[test]
fn a_wash_is_one_extra_visual_parked_at_zero() {
    let mut patch = fixture();
    let _mount = mount(
        plate().state(StatePolicy::Wash {
            hover: Wash::Ink,
            press: Wash::Ink,
        }),
        root(),
    );
    flush(&mut patch);

    let sprites = patch
        .ops()
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::New {
                    kind: windows_scene::NodeKind::Sprite,
                    ..
                }
            )
        })
        .count();
    assert_eq!(sprites, 2, "the fill, and the wash over it");

    let parked = patch.ops().iter().any(|op| {
        matches!(
            op,
            Op::Bind {
                prop: windows_scene::Prop::Opacity,
                bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(v)),
                ..
            } if *v == 0.0
        )
    });
    assert!(
        parked,
        "a never-hovered wash must be invisible without animating there"
    );
}

/// A sprite's colour resolves through the palette at the scope its surface pushed.
#[test]
fn a_surface_elevates_the_scope_its_children_resolve_against() {
    let mut patch = fixture();
    let _mount = mount(
        stack((plate(), plate().elevate(Elevation::Raised).stack(plate()))),
        root(),
    );
    flush(&mut patch);

    let painted: Vec<Radiance> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Paint {
                paint: Paint::Solid(light),
                ..
            } => Some(*light),
            _ => None,
        })
        .collect();
    // A surface resolves its own chrome at the rung it pushed, so the same role paints one
    // colour inside the push and another outside it.
    assert_eq!(
        painted.len(),
        3,
        "the bare plate, the surface, and its child"
    );
    assert_ne!(
        painted[0], painted[1],
        "a scope push must reach the sprite that pushed it"
    );
    assert_eq!(
        painted[1], painted[2],
        "everything inside one push resolves at that rung"
    );
}

/// A `Metric` resolves through the palette on its way into the lowered style.
#[test]
fn a_metric_override_lowers_through_the_palette() {
    let scope = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    let style = crate::layout::lower(
        crate::layout::Preset::Bare,
        &[crate::layout::Rule::always(crate::layout::Over::Width(
            Len::Metric(Metric::CardMinW),
        ))],
        scope,
    );
    assert_eq!(
        style.size.width,
        taffy::Dimension::length(crate::role::metric(Metric::CardMinW, scope)),
        "the width must be whatever the palette said, and nothing else"
    );
}

/// Text is measured under the type ramp the palette resolved for its role.
///
/// The claim is the ratio between two rungs rather than an absolute width: the engine is
/// DirectWrite, so a figure written down here would pin the test to one font's advances.
#[test]
fn text_measures_under_the_resolved_type_ramp() {
    fn width_of(ramp: TypeRole) -> f32 {
        let mut patch = fixture();
        let label = El::<Any>::seed(crate::layout::Preset::Text).text_seed(
            crate::widget::TextSource::Static("hello"),
            ramp,
            Some(Role::Text(Text::Primary)),
            Flow::Line,
            false,
        );
        let _mount = mount(label, root());
        flush(&mut patch);
        Host::with(|h| {
            let (_, row) = h.mounts.iter().last().expect("the label mounted");
            let node = row.node;
            h.model().solved(node).size.x
        })
    }

    let scope = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    let body = crate::role::typography(TypeRole::Body, scope).size;
    let display = crate::role::typography(TypeRole::Display, scope).size;
    assert!(
        display > body,
        "the ramp under test does not separate its rungs"
    );

    let (measured_body, measured_display) = (width_of(TypeRole::Body), width_of(TypeRole::Display));
    assert!(measured_body > 0.0, "the body rung measured nothing");

    // One string, one face, two sizes: advances scale with the em, so the measured widths
    // carry the ramp's own ratio. The tolerance covers hinting, which quantizes advances
    // per size.
    let expected = display / body;
    let actual = measured_display / measured_body;
    assert!(
        (actual - expected).abs() < 0.08 * expected,
        "measured ratio {actual} against the ramp's {expected} \
         ({measured_display} / {measured_body})"
    );
}

/// Mounting twice reuses the arena rather than growing it: the second mount of the same
/// shape allocates nothing.
#[test]
fn the_arena_is_pooled_across_mounts() {
    let mut patch = fixture();
    let _mount = mount(stack((plate(), plate())), root());
    flush(&mut patch);
    let high_water = Build::with(|b| b.nodes.capacity());
    assert!(high_water > 0, "the arena kept its capacity");

    let _mount = mount(stack((plate(), plate())), root());
    flush(&mut patch);
    assert_eq!(
        Build::with(|b| b.nodes.capacity()),
        high_water,
        "a second mount of the same shape must not grow the arena"
    );
}

/// `Len` carries no arbitrary DIP, so a widget can express only the palette's lengths.
///
/// The exhaustive match below is the whole vocabulary: a raw-DIP variant added to `Len`
/// stops this compiling.
#[test]
fn len_has_no_raw_dip_constructor() {
    let scope = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    for len in [
        Len::Metric(Metric::SpaceMd),
        Len::Zero,
        Len::Pct(0.5),
        Len::Times(Metric::RowH, 4.0),
        Len::Auto,
    ] {
        // `Times` is a count of a metric, so it can say "four rows" and cannot say "twelve".
        match len {
            Len::Metric(_) | Len::Zero | Len::Pct(_) | Len::Times(..) | Len::Auto => {}
        }
        let _ = len.dimension(scope);
    }
    assert_eq!(
        Len::Times(Metric::RowH, 4.0).dips(scope),
        Some(crate::role::metric(Metric::RowH, scope) * 4.0)
    );
    assert_eq!(Len::Zero.dips(scope), Some(0.0));
    assert_eq!(
        Len::Auto.dips(scope),
        None,
        "auto is a question, not a length"
    );
}

/// Colour does not read the width axis, so a resize re-lowers styles and rebinds no paint.
///
/// Every role is checked at every elevation, polarity and width class.
#[test]
fn colour_is_width_independent() {
    let base = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    let roles = [
        Role::Text(Text::Primary),
        Role::Text(Text::Secondary),
        Role::Text(Text::Tertiary),
        Role::Text(Text::Disabled),
        Role::Text(Text::Accent),
        Role::Text(Text::OnAccent),
        Role::Fill(Fill::Surface),
        Role::Fill(Fill::Hover),
        Role::Fill(Fill::Pressed),
        Role::Fill(Fill::Selected),
        Role::Fill(Fill::Accent),
        Role::Fill(Fill::AccentSubtle),
        Role::Stroke(Stroke::Subtle),
        Role::Stroke(Stroke::Default),
        Role::Stroke(Stroke::Focus),
        Role::Stroke(Stroke::Accent),
    ];
    for elevation in [
        Elevation::Base,
        Elevation::Raised,
        Elevation::Overlay,
        Elevation::Flyout,
    ] {
        for polarity in [Polarity::Dark, Polarity::Light] {
            let scope = Scope {
                elevation,
                polarity,
                ..base
            };
            for role in roles {
                let pinned = crate::role::resolve(role, scope.for_paint());
                for class in [
                    windows_scene::WidthClass::Narrow,
                    windows_scene::WidthClass::Medium,
                    windows_scene::WidthClass::Wide,
                ] {
                    assert_eq!(
                        pinned,
                        crate::role::resolve(role, scope.at_width(class).for_paint()),
                        "{role:?} moved with the width class"
                    );
                }
            }
        }
    }
}

/// A surface arranges its children in the class it was given, whatever chrome it carries.
///
/// Chrome and layout class are separate fields, so the class always wins: a card whose chrome
/// is a column still lays `card().row(..)` out along x.
#[test]
fn a_surface_arranges_its_children_as_it_was_told() {
    let mut patch = fixture();
    let plates = || {
        (
            plate().width(Metric::CardMinW).height(Metric::CardMinH),
            plate().width(Metric::CardMinW).height(Metric::CardMinH),
        )
    };
    let _mount = mount(
        El::<Any>::seed(crate::layout::Preset::Bare)
            .surface(
                Elevation::Raised,
                crate::widget::roles::SURFACE_CARD,
                Metric::Radius,
            )
            .row(plates()),
        root(),
    );
    flush(&mut patch);

    // Mounts are pushed in walk order, so the surface is first and its two children follow.
    let (a, b) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[1]), h.model().solved(nodes[2]))
    });
    assert!(
        b.rect.x0 > a.rect.x0,
        "a surface that rows must lay its children out along x, not down y"
    );
    assert!(
        (b.rect.y0 - a.rect.y0).abs() < 0.5,
        "children of a row share a baseline"
    );
}

/// A surface keeps its padding, scope push and fill whichever layout class it takes.
#[test]
fn a_surface_keeps_its_chrome_whichever_class_it_takes() {
    let mut patch = fixture();
    let _mount = mount(
        El::<Any>::seed(crate::layout::Preset::Bare)
            .surface(
                Elevation::Raised,
                crate::widget::roles::SURFACE_CARD,
                Metric::Radius,
            )
            .row(plate().width(Metric::CardMinW).height(Metric::CardMinH)),
        root(),
    );
    flush(&mut patch);

    let scope = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    let padding = crate::role::metric(Metric::SpaceLg, scope);
    let (surface, child) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[0]), h.model().solved(nodes[1]))
    });
    assert!(
        (child.rect.x0 - surface.rect.x0 - padding).abs() < 0.5,
        "the surface's padding must survive being told to be a row"
    );
    // The surface's own fill seed survives too. The rung it resolves at is asserted by
    // `a_surface_elevates_the_scope_its_children_resolve_against`.
    let painted = patch
        .ops()
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Paint {
                    paint: Paint::Solid(_),
                    ..
                }
            )
        })
        .count();
    // A card's hairline is an outer box in the stroke colour with the fill inset over it,
    // because the sprite alphabet has no outlined rectangle: two paints for the card and
    // one for the child.
    assert_eq!(painted, 3, "the card's ring and fill, and the child's");
}

/// Motion is declared by the seed, so a channel's default is the same at every call site.
#[test]
fn motion_is_per_channel_and_not_per_call_site() {
    assert_eq!(Motion::default(), Motion::Snap);
}

// ── unmount ──────────────────────────────────────────────────────────────────────

/// Dropping a mount releases every row the walk claimed.
///
/// The scene nodes go away regardless, so a retained control row, style recipe or shaped run
/// shows up only as a table that grows for the life of the process.
#[test]
fn unmounting_releases_every_row_it_claimed() {
    let mut patch = fixture();
    let mount = mount(crate::widget::button("Save"), root());
    flush(&mut patch);

    let (mounts, controls, runs) = Host::with(|h| {
        (
            h.mounts.len(),
            h.controls.len(),
            text::with(|t| t.entries.len()),
        )
    });
    assert!(mounts > 0 && controls == 1 && runs == 1);

    drop(mount);
    flush(&mut patch);
    let (mounts, controls, runs) = Host::with(|h| {
        (
            h.mounts.len(),
            h.controls.len(),
            text::with(|t| t.entries.len()),
        )
    });
    assert_eq!(
        (mounts, controls, runs),
        (0, 0, 0),
        "an unmount must release the style rows, the control rows and the runs"
    );
    // Every other table the walk claimed is released through the row that named it rather
    // than through a scan, so unmounting one list row costs that row and not the screen.
    Host::with(|h| {
        assert_eq!(h.scrolls.len(), 0);
    });
    assert_eq!(
        style::with(|table| table.len()),
        0,
        "an unmount must release the style recipes"
    );

    // One destroy op: it cascades on the far side, so a subtree cannot be half-gone.
    let drops = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::Drop { .. }))
        .count();
    assert_eq!(drops, 1);
}

/// Released slots are reused, so a list that churns does not grow the tables.
#[test]
fn released_rows_are_reused_rather_than_appended() {
    let mut patch = fixture();
    for _ in 0..8 {
        let mount = mount(crate::widget::button("x"), root());
        flush(&mut patch);
        drop(mount);
        flush(&mut patch);
    }
    let (mounts, controls) = Host::with(|h| (h.mounts.slots(), h.controls.slots()));
    assert_eq!(
        controls, 1,
        "eight mounts of one control must occupy one control slot, not eight"
    );
    assert!(
        mounts <= 2,
        "a button is one node and its label, so eight mounts must reuse the same rows"
    );
}

// ── variants ─────────────────────────────────────────────────────────────────────

/// A variant is a row, and the row decides how many sprites are minted.
///
/// A ghost declares no fill and no stroke, so it mints neither rather than minting two
/// invisible sprites.
#[test]
fn a_variant_row_decides_what_is_minted() {
    fn sprites(patch: &SinkPatch) -> usize {
        patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::New {
                        kind: windows_scene::NodeKind::Sprite,
                        ..
                    }
                )
            })
            .count()
    }

    let mut patch = fixture();
    let _default = mount(crate::widget::button("x"), root());
    flush(&mut patch);
    let full = sprites(&patch);
    patch.clear();

    let _ghost = mount(crate::widget::button("x").ghost(), root());
    flush(&mut patch);
    assert!(
        sprites(&patch) < full,
        "a ghost must cost the sprites it does not have: {} against {full}",
        sprites(&patch)
    );
}

// ── value controls ───────────────────────────────────────────────────────────────

/// A control claims the moving part its children declared, and the room the solve measured.
///
/// The thumb is a child of the control, so the front-side row is where the router finds it.
/// A row with no thumb leaves the router computing a value it cannot show, which is a slider
/// that renders and fires its handlers while nothing moves.
#[test]
fn a_control_claims_the_moving_part_its_children_declared() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.5_f64);
        let _slider = mount(
            crate::widget::slider(
                value,
                crate::widget::Range::UNIT,
                crate::widget::SliderStyle::default(),
            )
            .width(Metric::CardMinW),
            root(),
        );
        flush(&mut patch);

        let front = Host::with(|h| {
            h.controls
                .iter()
                .next()
                .map(|(_, c)| c.front)
                .expect("the slider minted a control")
        });
        assert!(
            front.thumb.is_some(),
            "the front row must name the part it is expected to move"
        );
        assert!(
            front.travel > 0.0,
            "and the room layout measured for it: {}",
            front.travel
        );
    });
}

/// A fraction is multiplied by the travel before it reaches the offset.
///
/// `Prop::OffsetX` is in DIPs, so a `0..=1` fraction bound to it raw moves a thumb by one DIP.
///
/// A toggle's knob is finished on this thread: a press reads no value off the pointer, so the
/// knob follows the application's own channel and the app thread is its writer.
#[test]
fn a_fraction_reaches_the_offset_multiplied_by_its_room() {
    let mut patch = fixture();
    let on = crate::signal::Cell::new(true);
    let _toggle = mount(crate::widget::toggle(on).width(Metric::CardMinW), root());
    flush(&mut patch);

    let (rest, travel) = Host::with(|h| {
        h.controls
            .iter()
            .next()
            .map_or((0.0, 0.0), |(_, c)| (c.front.rest, c.front.travel))
    });
    assert!(travel > 0.0, "a knob in a sized track has room to move");
    let front = Host::with(|h| h.controls.iter().next().unwrap().1.front);
    assert_eq!(front.source_fraction, 1.0);
    assert!(rest + travel > rest);
    assert!(binds(&patch, windows_scene::Prop::OffsetX).is_empty());
}

/// A part the router drives is not written from this thread after its mount seed.
///
/// The channel has one writer: the app thread ships the room the solve measured and the
/// front side multiplies. A second writer here would correct geometry against a live drag,
/// snapping the thumb back to the application's last value mid-slide.
#[test]
fn a_slid_part_is_left_to_the_thread_that_moves_it() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        // A slid part: its property is an offset finished against the room the solve gives.
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.25_f64);
        let _slider = mount(
            crate::widget::slider(
                value,
                crate::widget::Range::UNIT,
                crate::widget::SliderStyle::default(),
            )
            .width(Metric::CardMinW),
            root(),
        );
        flush(&mut patch);
        let front = Host::with(|h| {
            h.controls
                .iter()
                .next()
                .map(|(_, c)| c.front)
                .expect("a slider is a control")
        });
        assert!(
            front.travel > 0.0 && front.thumb.is_some(),
            "the router is shipped the part and the room it moves in"
        );
        assert_eq!(
            front.fraction, 0.25,
            "the initial value reaches the input owner"
        );
        // The mount seeds the part, because a control renders at its value before the router has
        // anything to report.
        assert!(binds(&patch, windows_scene::Prop::OffsetX).is_empty());

        // From here the router owns it: an application write to the same cell, which is what
        // `on_commit` does, must not reach the property.
        patch.clear();
        value.set(0.75);
        crate::signal::flush();
        flush(&mut patch);
        assert!(
            binds(&patch, windows_scene::Prop::OffsetX).is_empty(),
            "an owned part must not be written from this thread"
        );
        let changed = Host::with(|h| h.control(front.id).unwrap().front);
        assert_eq!(
            changed.fraction, 0.75,
            "an external edit reaches the input owner"
        );
        // The number it will land on is still this thread's own, from the same function.
        assert!(
            (crate::widget::offset_of(1.0, front.travel, false) - front.travel).abs()
                < f32::EPSILON
        );

        // A turned part: its property is an angle, finished through the same function.
        let mut patch = fixture();
        let angle = crate::signal::Cell::new(0.25_f64);
        let _knob = mount(
            crate::widget::knob(angle, crate::widget::Range::UNIT).width(Metric::CardMinW),
            root(),
        );
        flush(&mut patch);
        patch.clear();
        angle.set(0.75);
        crate::signal::flush();
        flush(&mut patch);
        assert!(
            binds(&patch, windows_scene::Prop::RotationAngle).is_empty(),
            "an owned angle must not be written from this thread either"
        );
    });
}

/// Returns every op this thread bound to `want`, whatever the binding kind.
fn binds(patch: &SinkPatch, want: windows_scene::Prop) -> Vec<&Op> {
    patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::Bind { prop, .. } if *prop == want))
        .collect()
}

/// Returns every scalar `OffsetX` this thread set, in the order it set them.

/// A read-only widget declares no hit entry, and therefore no control row.
///
/// Meters are dense on screen, and each hit entry is one more rect every pointer sample is
/// resolved against, plus a control row and a front-side row.
#[test]
fn a_meter_is_not_a_control() {
    let mut patch = fixture();
    let level = crate::signal::Cell::new(0.4_f32);
    let _meter = mount(crate::widget::meter(level), root());
    flush(&mut patch);

    let controls = Host::with(|h| h.controls.len());
    assert_eq!(controls, 0, "a meter must mint no control row");
    let entries = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Hits { entries } => Some(entries.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert_eq!(entries, 0, "and contribute nothing to the hit array");
}

// ── presence ─────────────────────────────────────────────────────────────────────

/// A constant `.when(false)` contributes nothing: no node, no style, no shaped run.
///
/// Hiding the element instead would cost a visual, a style, a mount row and, for a label,
/// a shaped run.
#[test]
fn a_constantly_absent_element_is_never_mounted() {
    let mut patch = fixture();
    let _screen = mount(
        stack((
            plate(),
            crate::widget::text("gone").when(false),
            plate().when(true),
        )),
        root(),
    );
    flush(&mut patch);

    let minted = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::New { .. }))
        .count();
    assert_eq!(minted, 3, "the container and the two present children");
    assert_eq!(
        text::with(|t| t.entries.len()),
        0,
        "an absent label must not shape its string"
    );
}

// ── structure ────────────────────────────────────────────────────────────────────

/// A keyed list reorders survivors rather than reminting them, so a reorder is moves only.
///
/// A filter keystroke therefore costs one move per row that changed place, not a rebuilt
/// subtree.
#[test]
fn a_keyed_list_moves_survivors_rather_than_reminting_them() {
    let mut patch = fixture();
    let items = crate::signal::Cell::new(vec![1_u32, 2, 3]);
    let _list = mount(
        stack(each(move || items.get(), |item| item, |_| plate())),
        root(),
    );
    flush(&mut patch);
    let minted = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::New { .. }))
        .count();
    assert!(minted >= 4, "a group and three rows: {minted}");
    patch.clear();

    // The same three keys, reordered. Nothing is built and nothing is destroyed.
    items.set(vec![3, 1, 2]);
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        !patch
            .ops()
            .iter()
            .any(|op| matches!(op, Op::New { .. } | Op::Drop { .. })),
        "a reorder must not mint or destroy a node: {:?}",
        patch.ops()
    );
    assert!(
        patch.ops().iter().any(|op| matches!(op, Op::Move { .. })),
        "and it must actually move one"
    );
}

/// `when(false)` contributes nothing: no node, no layout participation, no placeholder.
#[test]
fn an_absent_branch_mints_nothing() {
    let mut patch = fixture();
    let showing = crate::signal::Cell::new(false);
    let _branch = mount(stack(when(showing, plate)), root());
    flush(&mut patch);
    let minted = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::New { .. }))
        .count();
    // The container and the adapter's own group, and nothing for the absent arm.
    assert_eq!(minted, 2, "an absent arm must contribute no node");

    patch.clear();
    showing.set(true);
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        patch.ops().iter().any(|op| matches!(op, Op::New { .. })),
        "and it must arrive when the condition does"
    );
}

// ── text ─────────────────────────────────────────────────────────────────────────

/// A run that can break costs one sprite per line; a single-line run costs one sprite.
///
/// A coverage tile covers one line, so a wrapping caption needs several, and a `Flow::Line`
/// run needs no group behind it.
#[test]
fn only_a_wrapping_run_costs_a_sprite_per_line() {
    let mut patch = fixture();
    let _label = mount(crate::widget::text("a short label"), root());
    flush(&mut patch);
    let minted = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::New { .. }))
        .count();
    assert_eq!(minted, 1, "a non-wrapping run is one visual");
}

/// Collecting a container's children uses the arena's own buffers, not a temporary `Vec`.
///
/// The mount walk runs for every row a list realizes during a fling and may not allocate,
/// and a per-container temporary would cost one allocation per container per mount.
#[test]
fn collecting_children_uses_the_arenas_own_buffer() {
    let mut patch = fixture();
    let screen = || {
        stack((
            stack((plate(), plate(), plate())),
            stack((plate(), plate())),
            plate(),
        ))
    };

    let first = mount(screen(), root());
    flush(&mut patch);
    let (kids, pending) = Build::with(|b| (b.kids.capacity(), b.pending.capacity()));
    assert!(
        kids > 0 && pending > 0,
        "the child list and the stack it is collected on are both the arena's"
    );
    // Nothing is left standing on the stack once every container has taken its run.
    assert_eq!(Build::with(|b| b.pending.len()), 0);
    drop(first);

    // The same shape again grows neither buffer, so a realized row costs the walk alone.
    let second = mount(screen(), root());
    flush(&mut patch);
    assert_eq!(
        Build::with(|b| (b.kids.capacity(), b.pending.capacity())),
        (kids, pending),
        "a second mount of the same shape must not grow the arena"
    );
    drop(second);
}

/// A warm mount allocates nothing, measured with the allocation counter.
///
/// A capacity check cannot see a temporary allocated and freed inside the call, so the count
/// is what this asserts on. The counter is per thread and read either side of the mount
/// statement, so it reports this mount's own allocations.
#[test]
fn a_warm_mount_allocates_nothing() {
    let mut patch = fixture();
    let screen = || {
        crate::widget::card().stack((
            crate::widget::title("Effects"),
            stack((plate(), plate(), plate())),
            crate::widget::button("Apply"),
        ))
    };

    // The first mount grows the arena, the tables and the shaper's own buffers to high-water
    // mark, which is the once-per-shape cost the count below excludes.
    let warm = mount(screen(), root());
    flush(&mut patch);
    drop(warm);
    flush(&mut patch);

    let before = crate::counting::allocations();
    let second = mount(screen(), root());
    let during = crate::counting::allocations() - before;
    flush(&mut patch);
    drop(second);

    assert_eq!(
        during, 0,
        "a warm mount allocated {during} times; the arena exists to make that zero"
    );
}

/// Explicit grid placement appends into the arena's buffer rather than through a temporary.
#[test]
fn explicit_placement_appends_without_a_temporary() {
    let mut patch = fixture();
    let grid = crate::layout::grid(())
        .at(0, 0, plate())
        .at(0, 1, plate())
        .at(1, 0, plate());
    let _mount = mount(grid, root());
    flush(&mut patch);

    let minted = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::New { .. }))
        .count();
    assert_eq!(minted, 4, "the grid and its three placed cells");
    assert_eq!(Build::with(|b| b.pending.len()), 0);
}

// ── scroll ───────────────────────────────────────────────────────────────────────

/// A scroll container binds its content and its thumb to one tracker, and never the viewport.
///
/// The viewport carries the clip, so an offset on it would move the clip with the content.
/// The thumb rides the same tracker, so no frame positions it from the app thread.
#[test]
fn a_scroll_container_binds_its_content_and_its_thumb_to_one_tracker() {
    let mut patch = fixture();
    let tall = || {
        plate()
            .height(Metric::CardMinH)
            .min_height(Metric::CardMinH)
    };
    let _scroll = mount(
        crate::layout::scroll((tall(), tall(), tall(), tall(), tall(), tall()))
            .height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);

    let tracked: Vec<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Bind {
                id,
                bind: windows_scene::Bind::Track { tracker, .. },
                ..
            } => Some((*id, *tracker)),
            _ => None,
        })
        .collect();
    assert_eq!(
        tracked.len(),
        2,
        "the content and the thumb, and nothing else"
    );
    assert_eq!(
        tracked[0].1, tracked[1].1,
        "both must ride the same tracker, or the thumb reports on something else"
    );

    let viewport = Host::with(|h| h.mounts.iter().next().map(|(_, m)| m.node));
    assert!(
        tracked.iter().all(|(id, _)| Some(*id) != viewport),
        "the viewport clips, so it must not be the thing that moves"
    );

    // The extent reached the tracker, and it came from the solve.
    assert!(
        patch.ops().iter().any(|op| matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Bounds { max, .. },
                ..
            } if max.y > 0.0
        )),
        "content taller than its viewport must give the tracker somewhere to go"
    );
}

/// A second flush with nothing moved re-publishes nothing.
///
/// Scrolling moves compositor-side, and this step emits only when the extents change, so a
/// scroll in flight costs the app thread nothing.
#[test]
fn a_settled_scroll_container_emits_nothing() {
    let mut patch = fixture();
    let _scroll = mount(
        crate::layout::scroll(plate().height(Metric::CardMinH)).height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);
    patch.clear();
    flush(&mut patch);
    assert!(
        patch.is_empty(),
        "a settled scroll container emitted: {:?}",
        patch.ops()
    );
}

/// Content taller than its viewport overflows, whether or not its children pin a minimum.
///
/// A flex child shrinks to its parent by default, so a scroll container's content opts out
/// of that shrink; otherwise a child stating only a height leaves the container no travel.
#[test]
fn a_scroll_containers_content_is_not_squeezed_into_its_viewport() {
    let mut patch = fixture();
    let card = || plate().height(Metric::CardMinH);
    let _scroll = mount(
        crate::layout::scroll((card(), card(), card(), card(), card(), card()))
            .height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);
    assert!(
        patch.ops().iter().any(|op| matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Bounds { max, .. },
                ..
            } if max.y > 0.0
        )),
        "six cards in a one-card viewport gave the tracker nowhere to go"
    );
}

/// A scroll container's tracker is created, and created after its viewport is sized.
///
/// A tracker that is only minted is a binding onto nothing. One created before the solve
/// takes its hit region from a zero-size visual, which hit-tests nothing while reporting
/// success, so the surface ignores every wheel notch for the life of the window.
#[test]
fn a_scroll_containers_tracker_is_created_after_its_viewport_is_sized() {
    let mut patch = fixture();
    let _scroll = mount(
        crate::layout::scroll(plate().height(Metric::CardMinH)).height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);

    let viewport = Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .map(|(_, row)| row.viewport)
            .expect("a scroll container was mounted")
    });
    let created = patch.ops().iter().position(|op| {
        matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Create { .. },
                ..
            }
        )
    });
    let sized = patch.ops().iter().position(|op| {
        matches!(
            op,
            Op::Bind {
                id,
                prop: windows_scene::Prop::Size,
                ..
            } if *id == viewport
        )
    });
    let created = created.expect("the tracker was minted and never created");
    let sized = sized.expect("the viewport was never sized");
    assert!(
        sized < created,
        "the tracker was created at op {created}, before its viewport was sized at {sized}"
    );
}

/// The scrollbar is minted above the content, wins the hit array, and does not scroll.
///
/// Child order is paint order and the order the hit array is scanned in, so a bar minted at
/// the bottom of its viewport is painted under the list and every grab on it resolves to the
/// row behind it. The rail sits inside the container it reports on, so a rect resolved
/// through that container's offset slides off the surface as far as the content scrolls.
#[test]
fn the_scrollbar_is_above_the_content_grabbable_and_pinned() {
    let mut patch = fixture();
    // One card is a target, so the hit array carries an entry that does resolve through the
    // viewport's offset for the rail's opt-out to be measured against.
    let card = || plate().height(Metric::CardMinH);
    let _scroll = mount(
        crate::layout::scroll((card().on_click(|| {}), card(), card(), card()))
            .height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);

    let (content, rail, viewport, grab) = Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .map(|(_, row)| {
                (
                    row.content,
                    row.rail.expect("an on-demand scrollbar has a rail").node(),
                    row.control.expect("the viewport is a control"),
                    row.grab.expect("the rail is a control"),
                )
            })
            .expect("a scroll container was mounted")
    });

    // Paint order: minted above the content rather than at the bottom of the viewport.
    assert!(
        patch.ops().iter().any(|op| matches!(
            op,
            Op::New { id, after: Some(after), .. } if *id == rail && *after == content
        )),
        "the scrollbar was minted under the content it reports on"
    );

    // Hit order: the array is scanned from the end, so a later entry wins a point both cover,
    // and the rail sits inside the viewport's own box everywhere.
    let entries = patch.hit_entries();
    let entry = |id| entries.iter().position(|e| e.id == id);
    let rail_at = entry(grab).expect("the rail is not in the hit array");
    let viewport_at = entry(viewport).expect("the viewport is not in the hit array");
    assert!(
        rail_at > viewport_at,
        "a grab on the bar resolves to the surface behind it"
    );
    assert_eq!(
        entries[rail_at].scroll_src,
        windows_scene::NodeId::NONE,
        "the rail moves with the content it reports on"
    );
    // The card beside it does resolve through the offset, so the rail's flag is an opt-out.
    let scrolled = entries
        .iter()
        .filter(|entry| entry.scroll_src != windows_scene::NodeId::NONE)
        .count();
    assert!(
        scrolled > 0,
        "nothing in this container resolves through its offset, so the rail opted out of \
         nothing"
    );
}

/// A surface with nothing to scroll declares no rail hit entry.
///
/// A rail entry left in place takes every press on the right edge of the content, where no
/// scrollbar is drawn.
#[test]
fn a_surface_that_does_not_overflow_has_no_grab_target() {
    let mut patch = fixture();
    let _scroll = mount(
        crate::layout::scroll(plate().height(Metric::SpaceLg)).height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);

    let grab = Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .and_then(|(_, row)| row.grab)
            .expect("the rail is a control")
    });
    assert!(
        !patch.hit_entries().iter().any(|entry| entry.id == grab),
        "content that fits still put a scrollbar over its right edge"
    );
}

/// An on-demand thumb is bound to zero opacity at mount rather than shown and faded out.
///
/// Content that fits never overflows, so a thumb visible for the first frame is a flash on
/// every screen that opens.
#[test]
fn an_on_demand_thumb_starts_concealed() {
    let mut patch = fixture();
    let _scroll = mount(
        crate::layout::scroll(plate().height(Metric::CardMinH)).height(Metric::CardMinH),
        root(),
    );
    flush(&mut patch);
    assert!(
        patch.ops().iter().any(|op| matches!(
            op,
            Op::Bind {
                prop: windows_scene::Prop::Opacity,
                bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(v)),
                ..
            } if *v == 0.0
        )),
        "the thumb was mounted visible"
    );
}

/// A moved pixel grid re-sends every run, and a settled publish sends none.
///
/// Neither a pixel-grid change nor a rebuilt device moves a DIP, so the width gate that makes
/// the ordinary publish cheap reports nothing to do while every coverage tile is rasterized
/// for a grid that is gone.
#[test]
fn a_moved_pixel_grid_re_sends_every_run() {
    let mut patch = fixture();
    let _held = mount(crate::widget::label("re-emit me"), root());
    flush(&mut patch);

    let runs = |patch: &SinkPatch| {
        patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Res {
                        op: windows_scene::ResOp::Run { .. },
                        ..
                    }
                )
            })
            .count()
    };
    assert!(runs(&patch) > 0, "the label never emitted a run at all");

    // Settled: the ordinary publish is silent.
    patch.clear();
    flush(&mut patch);
    assert_eq!(runs(&patch), 0, "a settled label re-published its run");

    // A grid that moved is not.
    patch.clear();
    Host::with(Host::reemit_text);
    flush(&mut patch);
    assert!(
        runs(&patch) > 0,
        "a re-emit sent nothing, so a display hop leaves every glyph at the old resolution"
    );
}

/// Every control declares a gesture, and nothing else does.
///
/// `control()` sets `HitFlags::GESTURE`, which claims a gesture declaration behind the entry.
/// The router binds a contact only where its target declared one and reports an up only where
/// it bound, so an entry claiming a declaration it does not have is a press with no release.
///
/// The walk also runs for nodes that exist only for automation, where a recogniser has no
/// consumer, so a label declares none.
#[test]
fn a_control_declares_the_default_gesture_and_a_label_declares_none() {
    let mut patch = fixture();
    let _held = mount(crate::widget::button("press me"), root());
    flush(&mut patch);
    let declared = filled().gestures;
    assert_eq!(
        declared.len(),
        1,
        "a plain button declared {} gestures, so its press cannot be released",
        declared.len()
    );
    assert_eq!(
        declared[0].1,
        crate::gesture::GestureDecl::default(),
        "a control that refined nothing got something other than the default"
    );
    assert!(
        patch
            .hit_entries()
            .iter()
            .any(|entry| entry.flags.contains(windows_scene::HitFlags::GESTURE)),
        "the entry does not claim the declaration that was just made for it"
    );

    let mut patch = fixture();
    let _held = mount(crate::widget::label("just words"), root());
    flush(&mut patch);
    assert!(
        filled().gestures.is_empty(),
        "a static label was given a recogniser it can never use"
    );
}

// ── virtualization ───────────────────────────────────────────────────────────────

/// Specifies a thousand-row list, in a viewport that shows about ten of them.
const LIST: crate::layout::ListSpec = crate::layout::ListSpec {
    count: 1000,
    row_h: Metric::RowH,
    overscan: 2,
};

/// Settles a mounted list and returns the tracker driving it.
///
/// Two flushes, because a viewport's height is a solve output: the first flush measures it
/// and the realization window it implies is resolved on the tick after. A running window
/// mounts and resizes the same way.
fn settle(patch: &mut SinkPatch) -> windows_scene::Id<windows_scene::Tracker> {
    flush(patch);
    crate::signal::flush();
    flush(patch);
    Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .map(|(_, row)| row.tracker.id())
            .expect("the list mounted a scroll container")
    })
}

/// Mounts a virtualized list and returns the tracker driving it.
fn virtualized(patch: &mut SinkPatch) -> windows_scene::Id<windows_scene::Tracker> {
    let _held = mount(
        crate::layout::list(
            || LIST,
            |realized, out| {
                for run in realized.runs() {
                    out.extend(run.map(|index| (index, index)));
                }
            },
            |index: &usize| plate().name(if *index == 0 { "first" } else { "row" }),
        )
        .height(Metric::CardMinH),
        root(),
    )
    .leak();
    settle(patch)
}

/// Returns how many rows the list realized, counted off the nodes the mount walk claimed.
fn realized_rows() -> usize {
    Host::with(|h| h.mounts.iter().count())
}

/// Returns the travel the solve gave the tracker, read off the scroll row's last publish.
///
/// A flush replaces the caller's buffer rather than appending to it, and the extent settles
/// on the flush that measured the viewport rather than on the one after it, so the row is the
/// only place it survives.
fn published_extent() -> f32 {
    Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .map(|(_, row)| row.last.max_scroll)
            .expect("a scroll container was mounted")
    })
}

/// A thousand rows cost a screen's worth of nodes, each placed at its own index.
///
/// Placement by index lets the realized set be several disjoint runs, and keeps the content's
/// extent the whole list's, so the maximum position does not move as the window does.
#[test]
fn a_virtualized_list_realizes_a_screen_and_places_what_it_realized() {
    let mut patch = fixture();
    virtualized(&mut patch);

    let rows = realized_rows();
    assert!(
        (5..40).contains(&rows),
        "a thousand-row list realized {rows} nodes"
    );

    let row_h = crate::role::metric(Metric::RowH, Host::with(|h| h.root_scope));
    let offsets: Vec<f32> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Bind {
                prop: windows_scene::Prop::Offset,
                bind: windows_scene::Bind::Set(windows_scene::Value::Vec2(at)),
                ..
            } => Some(at.y),
            _ => None,
        })
        .collect();
    // Every row sits on a row-height boundary. A sequence of laid-out children would also
    // reach past the fifth row, so the boundary is what separates placement from layout.
    assert!(
        offsets.iter().any(|y| *y > row_h * 4.0),
        "no row was placed past the fifth: {offsets:?}"
    );
    for y in &offsets {
        let index = y / row_h;
        assert!(
            (index - index.round()).abs() < 0.01,
            "a row landed off its own boundary at {y}"
        );
    }

    // The extent the tracker was given is the whole list's, not the realized set's.
    let max = published_extent();
    assert!(
        max > row_h * (LIST.count as f32) * 0.9,
        "the tracker's travel was {max}, which is not a thousand rows"
    );
}

/// A reported position realizes the rows under it in the tick it arrived in, and leaves the
/// extent alone.
///
/// A content height that followed the realized set would move the maximum position on every
/// frame of a fling, sliding the content under the user's finger.
#[test]
fn a_reported_position_realizes_the_rows_under_it() {
    let mut patch = fixture();
    let tracker = virtualized(&mut patch);
    let before = realized_rows();
    patch.clear();

    let row_h = crate::role::metric(Metric::RowH, Host::with(|h| h.root_scope));
    crate::layout::scroll_observe(&[windows_scene::SceneEvent::TrackerValues {
        tracker,
        position: Vector2 {
            x: 0.0,
            y: row_h * 500.0,
        },
        scale: 1.0,
    }]);
    crate::signal::flush();
    flush(&mut patch);

    // A window at the very top has its upper overscan clipped away and one in the middle does
    // not, so the counts sit within the overscan of each other rather than equal. Neither
    // grows with how far the list was scrolled.
    let after = realized_rows();
    assert!(
        after.abs_diff(before) <= LIST.overscan,
        "a scrolled list realized {after} rows against {before} at the top"
    );
    assert!(
        !patch.ops().iter().any(|op| matches!(
            op,
            Op::Tracker {
                op: windows_scene::TrackerOp::Bounds { .. },
                ..
            }
        )),
        "scrolling moved the extent, so the maximum position is not a constant"
    );
    let placed = patch.ops().iter().any(|op| {
        matches!(
            op,
            Op::Bind {
                prop: windows_scene::Prop::Offset,
                bind: windows_scene::Bind::Set(windows_scene::Value::Vec2(at)),
                ..
            } if at.y > row_h * 400.0
        )
    });
    assert!(
        placed,
        "nothing was realized where the content had moved to"
    );
}

/// A fling realizes the rows at its destination while keeping the rows it is leaving.
///
/// The destination is known at the instant inertia begins, so those rows are realized while
/// the compositor animates. A window that moved to the destination instead would blank the
/// rows still on screen.
#[test]
fn a_fling_realizes_its_destination_without_dropping_where_it_is() {
    let mut patch = fixture();
    let tracker = virtualized(&mut patch);
    let resting = realized_rows();
    patch.clear();

    let row_h = crate::role::metric(Metric::RowH, Host::with(|h| h.root_scope));
    let landing = row_h * 500.0;
    crate::layout::scroll_observe(&[windows_scene::SceneEvent::InertiaStarting {
        tracker,
        natural: Vector2 { x: 0.0, y: landing },
        modified: Vector2 { x: 0.0, y: landing },
        from_impulse: false,
    }]);
    crate::signal::flush();
    flush(&mut patch);

    let flinging = realized_rows();
    assert!(
        flinging > resting,
        "a fling realized {flinging} rows against {resting} at rest — nothing was prefetched"
    );
    assert!(
        flinging < resting * 5,
        "a fling realized {flinging} rows, which is not a bounded corridor"
    );
    // Both ends exist at once: the destination was realized beside where the content still
    // is, not instead of it.
    let offsets: Vec<f32> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Bind {
                prop: windows_scene::Prop::Offset,
                bind: windows_scene::Bind::Set(windows_scene::Value::Vec2(at)),
                ..
            } => Some(at.y),
            _ => None,
        })
        .collect();
    assert!(
        offsets.iter().any(|y| *y > landing * 0.9),
        "nothing was realized where the fling lands: {offsets:?}"
    );

    // The prefetch is released once the tracker reports idle.
    patch.clear();
    crate::layout::scroll_observe(&[windows_scene::SceneEvent::TrackerPhase {
        tracker,
        phase: windows_scene::Phase::Idle,
    }]);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(
        realized_rows(),
        resting,
        "a settled list is still holding its destination"
    );
}

/// A realized index the caller did not supply reserves its space and holds nothing.
///
/// The placeholder sits where the row will be when the data arrives, so the extent and every
/// row below it are already right, and nothing invented stands in for the data.
#[test]
fn an_unsupplied_row_reserves_its_space() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::list(
            || LIST,
            // Nothing at all, at any position: the whole realized set is placeholders.
            |_, _: &mut Vec<(usize, usize)>| {},
            |index: &usize| plate().name(if *index == 0 { "first" } else { "row" }),
        )
        .height(Metric::CardMinH),
        root(),
    )
    .leak();
    settle(&mut patch);

    let rows = realized_rows();
    assert!(rows > 4, "a list of placeholders realized {rows} nodes");
    let row_h = crate::role::metric(Metric::RowH, Host::with(|h| h.root_scope));
    assert!(
        published_extent() > row_h * (LIST.count as f32) * 0.9,
        "a list of placeholders still has a thousand rows of extent"
    );
}

// ── modifier order ───────────────────────────────────────────────────────────────

/// `no_inflate` reads the same before or after the handler that declares a hit target, and
/// declares no target of its own.
///
/// The arena's intrusive chains make modifiers order-independent, so a chain written either
/// way round produces the same hit entry.
#[test]
fn declining_an_inflation_reads_the_same_in_either_order() {
    fn inflates(view: View) -> Option<bool> {
        let mut patch = fixture();
        let _held = mount(view, root());
        flush(&mut patch);
        // Read off the hit array, which is what the router consults.
        patch
            .hit_entries()
            .first()
            .map(|entry| !entry.flags.contains(windows_scene::HitFlags::NO_INFLATE))
    }
    assert_eq!(inflates(plate().no_inflate().on_click(|| {})), Some(false));
    assert_eq!(inflates(plate().on_click(|| {}).no_inflate()), Some(false));
    // On its own it is no target at all, so it costs neither a control row nor a slot in the
    // array every pointer sample is resolved against.
    assert_eq!(inflates(plate().no_inflate()), None);
}

/// A value handler declares the hit target it needs, so it reaches the dispatch table.
///
/// The mount moves handlers into the dense table only for a node that has a hit entry. A
/// handler on a node without one is freed with the arena and never reaches the table it is
/// dispatched from, which reads at the call site as a control that does nothing.
#[test]
fn a_value_handler_declares_the_target_it_needs() {
    let mut patch = fixture();
    let _held = mount(plate().on_change(|_| {}), root());
    flush(&mut patch);
    let has = Host::with(|h| {
        h.controls
            .iter()
            .next()
            .is_some_and(|(_, c)| c.change.is_some())
    });
    assert!(
        has,
        "the handler must reach the table it is dispatched from"
    );
}

// ── a style that follows a value follows its own scope ───────────────────────────

/// A restyle re-lowers against the node's own scope, not the root's.
///
/// A surface pushes a rung, and re-lowering from the root would lose the elevation silently,
/// because the answer is still a valid style.
#[test]
fn a_restyle_lowers_against_the_node_that_owns_it() {
    let mut patch = fixture();
    let shown = crate::signal::Cell::new(true);
    // Inside a card, so the scope the recipe carries is not the root's.
    let _held = mount(
        crate::widget::card().stack(plate().padding(Metric::SpaceLg).when(shown)),
        root(),
    );
    flush(&mut patch);

    let root_scope = Host::with(|h| h.root_scope);
    let elevated = style::with(|table| {
        table
            .iter()
            .find(|(_, recipe)| recipe.scope.elevation != root_scope.elevation)
            .map(|(node, _)| node)
    })
    .expect("a card elevates the scope its children resolve against");
    assert_eq!(
        style::with(|table| table.get(elevated).map(|recipe| recipe.scope)).map(|s| s.elevation),
        Some(root_scope.elevate(Elevation::Raised).elevation),
        "a restyle must read the node's own recipe rather than the root scope"
    );
}

/// The width class belongs to the solve and is never stored in the recipe re-lowering reads.
///
/// A second copy of the class in the recipe is a frame stale, so a container's own re-lower
/// would disagree with the layout it was laid out under.
#[test]
fn a_recipe_holds_no_width_class() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive([600.0, 1000.0], plate().padding(Metric::SpaceLg)),
        root(),
    );
    flush(&mut patch);

    let root_width = Host::with(|h| h.root_scope.width);
    assert!(
        style::with(|table| table
            .iter()
            .all(|(_, recipe)| recipe.scope.width == root_width)),
        "a recipe stored a resolved class, which is the copy that goes stale"
    );
}

// ── width variants ───────────────────────────────────────────────────────────────
//
// The fixture's window is 800 DIPs wide and the containers below fill it, so each test picks
// a class by moving the thresholds rather than the window width. The class under test is then
// readable from the thresholds at the call site.

/// A width variant re-arranges a container without unmounting anything.
///
/// The mount surviving is what makes a variant safe to evaluate during a resize drag: a
/// `when()` would drop the subtree's owner every time a window edge crossed the threshold.
#[test]
fn a_width_variant_re_arranges_without_unmounting() {
    let arrange = |bounds: [f32; 2]| {
        let mut patch = fixture();
        let plates = || {
            (
                plate().width(Metric::CardMinW).height(Metric::CardMinH),
                plate().width(Metric::CardMinW).height(Metric::CardMinH),
            )
        };
        let _held = mount(
            crate::layout::responsive(
                bounds,
                crate::layout::row(plates()).stack_when(windows_scene::WidthClass::Narrow),
            )
            .width(Len::Pct(1.0)),
            root(),
        );
        flush(&mut patch);
        // The classifier, the row inside it, then its two children.
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            let (a, b) = (h.model().solved(nodes[2]), h.model().solved(nodes[3]));
            (a.rect, b.rect, h.mounts.len())
        })
    };

    // 800 against [900, 1000] is Narrow; against [400, 600] it is Wide.
    let (narrow_a, narrow_b, narrow_mounts) = arrange([900.0, 1000.0]);
    let (wide_a, wide_b, wide_mounts) = arrange([400.0, 600.0]);

    assert!(
        narrow_b.y0 > narrow_a.y0 && (narrow_b.x0 - narrow_a.x0).abs() < 0.5,
        "at the narrow class the row must lay its children out down y"
    );
    assert!(
        wide_b.x0 > wide_a.x0 && (wide_b.y0 - wide_a.y0).abs() < 0.5,
        "outside it the same container must still be a row"
    );
    assert_eq!(
        narrow_mounts, wide_mounts,
        "a width variant changed the structure, which is the one thing it may not do"
    );
}

/// A single-line run's box is its own coverage, whatever its container does to its siblings.
///
/// A `Flow::Line` run has no line sprite of its own: the node is the sprite, which keeps a
/// static label at one visual. A container that stretched its children would stretch the
/// coverage tile with it, and the tile's brush fills, so a short label would smear
/// horizontally to the width of the longest line beside it. A wrapping run owns line sprites
/// and sizes each to its own tile, so it takes its width from the text either way.
///
/// The two runs are compared with each other rather than against an absolute width, so the
/// assertion needs no number from the text engine.
#[test]
fn a_single_line_run_is_as_wide_as_its_own_text() {
    let mut patch = fixture();
    // `stack` stretches its children, which is the default.
    let _held = mount(
        stack((
            crate::widget::text("a much longer line of text than the other one"),
            crate::widget::text("short"),
        ))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (long, short) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (
            h.model().solved(nodes[1]).size.x,
            h.model().solved(nodes[2]).size.x,
        )
    });
    assert!(
        short < long,
        "both runs were laid out at {long} DIPs, so the container's width reached the \
         coverage instead of the text's"
    );
    assert!(
        long < 800.0,
        "the longer run filled the container at {long} DIPs rather than measuring its text"
    );
}

/// A track sized by a fraction of its container is that fraction, not zero.
///
/// `Len::dips` answers `None` for the two lengths with no intrinsic value, a percentage and
/// `Auto`, so a fixed track resolves those against the container rather than collapsing to
/// zero. A grid with a collapsed track lays out cleanly and shows only content that is not
/// where it was placed.
#[test]
fn a_fractional_track_is_a_fraction_of_its_container() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            plate().height(Metric::CardMinH),
        ))
        .cols([Track::Fixed(Len::Pct(0.25)), Track::Fr(1.0)])
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (a, b) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[1]), h.model().solved(nodes[2]))
    });
    // A quarter of the fixture's 800-DIP window.
    assert!(
        (b.rect.x0 - a.rect.x0 - 200.0).abs() < 1.0,
        "a 25% track collapsed: the second child began {} DIPs across",
        b.rect.x0 - a.rect.x0
    );
}

/// A class-gated column list replaces the template below it rather than extending it.
///
/// `.cols(..).cols_when(..)` clears before it appends, so the wide arm holds two tracks and
/// not three. The assertion is on the second child's position, which is where a concatenated
/// track list shows up.
#[test]
fn a_class_gated_column_list_replaces_the_one_below_it() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            [400.0, 600.0],
            crate::layout::grid((
                plate().height(Metric::CardMinH),
                plate().height(Metric::CardMinH),
            ))
            .cols([Track::Fr(1.0)])
            .cols_when(
                windows_scene::WidthClass::Wide,
                [Track::Fr(1.0), Track::Fr(1.0)],
            )
            .gap(Len::Zero),
        )
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (a, b) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[2]), h.model().solved(nodes[3]))
    });
    let half = (b.rect.x0 - a.rect.x0 - 400.0).abs();
    assert!(
        half < 1.0,
        "the wide arm's two tracks must halve the container, not third it: the second \
         child began {} DIPs across",
        b.rect.x0 - a.rect.x0
    );
}

/// A computed column template follows the value it is computed from.
///
/// `cols_from` is the case `cols_if` cannot state: the first track's extent is a number, so
/// there is no condition to key an arm on and no finite set of arms to write. The assertion
/// is on where the second child begins, which is the first track's width — first at the value
/// the signal held at mount, then at the value it moved to.
#[test]
fn a_computed_column_template_follows_the_value_it_reads() {
    use crate::layout::Track;
    let rows = crate::signal::Cell::new(2.0_f32);
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            plate().height(Metric::CardMinH),
        ))
        .cols_from(move |out| {
            out.push(Track::Fixed(Len::Times(Metric::RowH, rows.get())));
            out.push(Track::Fr(1.0));
        })
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let second = || {
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            let (a, b) = (h.model().solved(nodes[1]), h.model().solved(nodes[2]));
            b.rect.x0 - a.rect.x0
        })
    };
    let row_h = crate::role::metric(
        Metric::RowH,
        crate::role::Scope::root(
            crate::role::tests::palette(),
            crate::role::AccentId(0),
            crate::role::Density::Comfortable,
        ),
    );
    assert!(
        (second() - row_h * 2.0).abs() < 1.0,
        "the computed track was not two row heights wide: the second child began {} DIPs \
         across, against {}",
        second(),
        row_h * 2.0
    );

    rows.set(5.0);
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        (second() - row_h * 5.0).abs() < 1.0,
        "the track did not follow its signal: the second child began {} DIPs across, against \
         {}",
        second(),
        row_h * 5.0
    );
}

#[test]
fn computed_rows_replace_the_template_without_remounting_children() {
    use crate::layout::{Track, grid};
    let mut patch = fixture();
    let fraction = crate::signal::Cell::new(0.25_f32);
    let _held = mount(
        grid((plate(), plate()))
            .rows([Track::Fr(1.0)])
            .rows_from(move |out| {
                out.extend([Track::Fixed(Len::Pct(fraction.get())), Track::Fr(1.0)])
            })
            .cols([Track::Fr(1.0)])
            .gap(Len::Zero)
            .height(Len::Pct(1.0))
            .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);
    let snapshot = || {
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            let a = h.model().solved(nodes[0]);
            let b = h.model().solved(nodes[2]);
            (nodes, (b.rect.y0 - a.rect.y0) / a.size.y)
        })
    };
    let (before, y) = snapshot();
    assert!((y - 0.25).abs() < 0.01);
    fraction.set(0.75);
    crate::signal::flush();
    flush(&mut patch);
    let (after, y) = snapshot();
    assert_eq!(before, after);
    assert!((y - 0.75).abs() < 0.01);
}

#[test]
fn keyed_tiles_fill_multiple_columns() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::tiles(
            Len::Times(Metric::CardMinW, 0.75),
            each_into(
                |out| out.extend(0..4),
                |i| i,
                |_| plate().min_width(Len::Zero),
            ),
        )
        .width(Len::Times(Metric::CardMinW, 2.0)),
        root(),
    );
    flush(&mut patch);
    let (a, b) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[2]), h.model().solved(nodes[3]))
    });
    assert!(
        b.rect.x0 > a.rect.x0,
        "tiles did not share a row: {a:?} {b:?}"
    );
    assert!((b.rect.y0 - a.rect.y0).abs() < 1.0);
}

/// A computed column template survives the container resolving its own width class.
///
/// The two lowerings — the one a bound style act pushes, and the one the solve asks for when
/// a class moves — both start from the node's recipe, so the recipe has to hold what the act
/// wrote. Without that, a classified container keeps its template only until the first solve
/// resolves a class other than the one the act ran at, and a two-track grid silently becomes
/// the single auto-placed column a grid with no template is.
///
/// The classifier is inside the grid's own subtree here, so the grid resolves a class at all;
/// the bounds put it at `Narrow`, which is not the class an unsolved node reads.
#[test]
fn a_computed_column_template_survives_a_class_change() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            [1000.0, 2000.0],
            crate::layout::grid((
                plate().height(Metric::CardMinH),
                plate().height(Metric::CardMinH),
            ))
            .cols_from(move |out| {
                out.push(Track::Fixed(Len::Times(Metric::RowH, 3.0)));
                out.push(Track::Fr(1.0));
            })
            .gap(Len::Zero)
            .width(Len::Pct(1.0)),
        )
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    // The classifier, the grid, then its two cells.
    let (first, second) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[2]), h.model().solved(nodes[3]))
    });
    assert_eq!(
        first.class,
        windows_scene::WidthClass::Narrow,
        "the fixture's window must classify narrow at these bounds for the test to mean          anything"
    );
    let row_h = crate::role::metric(
        Metric::RowH,
        crate::role::Scope::root(
            crate::role::tests::palette(),
            crate::role::AccentId(0),
            crate::role::Density::Comfortable,
        )
        .at_width(windows_scene::WidthClass::Narrow),
    );
    let track = second.rect.x0 - first.rect.x0;
    assert!(
        (track - row_h * 3.0).abs() < 1.0,
        "the computed template did not survive the class the solve resolved: the second          cell began {track} DIPs across, against {}",
        row_h * 3.0
    );
}

/// `hide_when` takes the node out of the layout and leaves it in the tree.
#[test]
fn hide_when_removes_the_box_and_not_the_node() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            [900.0, 1000.0],
            stack((
                plate().width(Metric::CardMinW).height(Metric::CardMinH),
                plate()
                    .width(Metric::CardMinW)
                    .height(Metric::CardMinH)
                    .hide_when(windows_scene::WidthClass::Narrow),
            )),
        )
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (hidden, mounts) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[3]), h.mounts.len())
    });
    assert!(
        hidden.size.x < 0.5 && hidden.size.y < 0.5,
        "a hidden part must occupy no space"
    );
    assert_eq!(
        mounts, 4,
        "hiding is a style, so the node it hid is still mounted"
    );
}

#[test]
fn unhandled_escape_follows_screen_visibility_and_mount_lifetime() {
    let mut patch = fixture();
    let hidden = crate::signal::Cell::new(false);
    let calls = std::rc::Rc::new(core::cell::Cell::new(0));
    let held = mount(
        stack(()).grow().hide_if(hidden).on_unhandled_escape({
            let calls = calls.clone();
            move || {
                // Re-entering the host is legal: the handler is taken out first.
                Host::with(|_| ());
                calls.set(calls.get() + 1);
            }
        }),
        root(),
    );
    flush(&mut patch);
    Host::with(|h| h.escape_handler()).unwrap()();
    assert_eq!(calls.get(), 1);
    hidden.set(true);
    crate::signal::flush();
    flush(&mut patch);
    assert!(Host::with(|h| h.escape_handler()).is_none());
    hidden.set(false);
    crate::signal::flush();
    flush(&mut patch);
    assert!(Host::with(|h| h.escape_handler()).is_some());
    drop(held);
    assert!(Host::with(|h| h.escape_handler()).is_none());
}

/// `float_when` takes the node out of flow, pins it to its edge and stretches the other axis.
///
/// The pinned node keeps its own width and the sibling beside it keeps the whole container,
/// which is what separates a float from a second column.
#[test]
fn float_when_pins_the_node_and_leaves_its_sibling_the_container() {
    use crate::layout::Edge;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            [900.0, 1000.0],
            crate::layout::stack((
                plate().height(Metric::CardMinH),
                plate()
                    .width(Metric::CardMinW)
                    .float_when(windows_scene::WidthClass::Narrow, Edge::Right),
            ))
            .gap(Len::Zero),
        )
        .width(Len::Pct(1.0))
        .height(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (lane, flow, floated) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (
            h.model().solved(nodes[1]),
            h.model().solved(nodes[2]),
            h.model().solved(nodes[3]),
        )
    });
    assert!(
        (floated.rect.x1 - lane.rect.x1).abs() < 0.5,
        "pinned to the right edge, so its trailing edge is the container's: {} against {}",
        floated.rect.x1,
        lane.rect.x1
    );
    assert!(
        floated.size.x < lane.size.x,
        "the float keeps its own width rather than filling the container"
    );
    assert!(
        (floated.size.y - lane.size.y).abs() < 0.5,
        "both insets are zero on the perpendicular axis, so it stretches: {} against {}",
        floated.size.y,
        lane.size.y
    );
    assert!(
        (flow.size.x - lane.size.x).abs() < 0.5,
        "an out-of-flow sibling takes no width from the one still in flow: {} against {}",
        flow.size.x,
        lane.size.x
    );
}

/// A float is not confined by the placement its container states, in either order.
///
/// The lane states `at(row, column)` for the docked case and the pane floats at the narrow
/// ones. Honouring both would seat the drawer in a track that exists only at another class.
#[test]
fn a_float_is_not_confined_by_its_containers_placement() {
    use crate::layout::{Edge, Track};
    /// Mounts a one-column grid whose second child is placed at column 1 and floats, and
    /// answers that child's solved box together with the grid's.
    fn boxes(float_first: bool) -> (windows_scene::Solved, windows_scene::Solved) {
        let mut patch = fixture();
        let pane = plate().width(Metric::CardMinW);
        // The two orders the override list can carry: the child's own rule is pushed where
        // it is written, and the container's `Place` when the child is added.
        let pane = if float_first {
            pane.float_when(windows_scene::WidthClass::Narrow, Edge::Right)
        } else {
            pane
        };
        let _held = mount(
            crate::layout::responsive(
                [900.0, 1000.0],
                crate::layout::grid(())
                    .at(0, 0, plate().height(Metric::CardMinH))
                    .at(
                        0,
                        1,
                        if float_first {
                            pane
                        } else {
                            pane.float_when(windows_scene::WidthClass::Narrow, Edge::Right)
                        },
                    )
                    .cols([Track::Fr(1.0)])
                    .gap(Len::Zero),
            )
            .width(Len::Pct(1.0))
            .height(Len::Pct(1.0)),
            root(),
        );
        flush(&mut patch);
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            (h.model().solved(nodes[1]), h.model().solved(nodes[3]))
        })
    }

    for float_first in [true, false] {
        let (lane, floated) = boxes(float_first);
        assert!(
            (floated.rect.x1 - lane.rect.x1).abs() < 0.5,
            "float_first={float_first}: the drawer landed at {} rather than the lane's own \
             trailing edge {}, so a placement confined it",
            floated.rect.x1,
            lane.rect.x1
        );
        assert!(
            (floated.size.y - lane.size.y).abs() < 0.5,
            "float_first={float_first}: the drawer is {} DIPs tall against the lane's {}, so \
             it was seated in a track rather than over the padding box",
            floated.size.y,
            lane.size.y
        );
    }
}

/// `float_below` floats at every class under its floor, and only there.
///
/// A class added to [`WidthClass`](windows_scene::WidthClass) must not leave the pane docked
/// in it.
#[test]
fn float_below_floats_every_class_under_its_floor() {
    use crate::layout::Edge;
    /// Mounts a part floating below `Wide` in a container whose 800-DIP width classifies
    /// against `bounds`, and answers whether that part is out of flow.
    fn floats(bounds: [f32; 2]) -> bool {
        let mut patch = fixture();
        let _held = mount(
            crate::layout::responsive(
                bounds,
                crate::layout::stack((
                    plate().height(Metric::CardMinH),
                    plate()
                        .width(Metric::CardMinW)
                        .float_below(windows_scene::WidthClass::Wide, Edge::Right),
                ))
                .gap(Len::Zero),
            )
            .width(Len::Pct(1.0))
            .height(Len::Pct(1.0)),
            root(),
        );
        flush(&mut patch);
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            let (lane, floated) = (h.model().solved(nodes[1]), h.model().solved(nodes[3]));
            // In flow the stack gives it the container's width and its own height; floated it
            // is the other way round.
            (floated.rect.x1 - lane.rect.x1).abs() < 0.5 && floated.size.x < lane.size.x
        })
    }

    // The fixture's window is 800 DIPs, so each set of bounds picks one class for it.
    assert!(
        floats([900.0, 1000.0]),
        "floating below Wide, so it must float at Narrow"
    );
    assert!(
        floats([600.0, 1000.0]),
        "floating below Wide, so it must float at Medium"
    );
    assert!(
        !floats([400.0, 600.0]),
        "the floor itself is not below it, so the pane docks at Wide"
    );
}

/// `hide_below` hides at every class under its floor, and only there.
///
/// A class added to [`WidthClass`](windows_scene::WidthClass) must not leave a subtree
/// visible in it.
#[test]
fn hide_below_hides_every_class_under_its_floor() {
    /// Mounts a part hidden below `Wide` in a container whose 800-DIP width classifies
    /// against `bounds`, and answers whether the part occupies space.
    fn shown(bounds: [f32; 2]) -> bool {
        let mut patch = fixture();
        let _held = mount(
            crate::layout::responsive(
                bounds,
                stack((
                    plate().width(Metric::CardMinW).height(Metric::CardMinH),
                    plate()
                        .width(Metric::CardMinW)
                        .height(Metric::CardMinH)
                        .hide_below(windows_scene::WidthClass::Wide),
                )),
            )
            .width(Len::Pct(1.0)),
            root(),
        );
        flush(&mut patch);
        Host::with(|h| {
            let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
            h.model().solved(nodes[3]).size.y > 0.5
        })
    }

    // The fixture's window is 800 DIPs, so each set of bounds picks one class for it.
    assert!(
        !shown([900.0, 1000.0]),
        "hidden below Wide, so it must not lay out at Narrow"
    );
    assert!(
        !shown([600.0, 1000.0]),
        "hidden below Wide, so it must not lay out at Medium"
    );
    assert!(
        shown([300.0, 400.0]),
        "the floor itself is not below the floor: it must lay out at Wide"
    );
}

/// A fractional track divides the container and is not floored by its own content.
///
/// A track floored at its content takes a scrolling column's full height as its minimum,
/// which collapses every other track in the template.
#[test]
fn a_fractional_track_is_not_floored_by_its_own_content() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            // Six card heights of content in a track entitled to half of four.
            stack([0; 6].map(|_| plate().height(Metric::CardMinH))),
        ))
        .rows([Track::Fr(1.0), Track::Fr(1.0)])
        .gap(Len::Zero)
        .height(Len::Times(Metric::CardMinH, 4.0))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    // Against the grid's own solved height rather than the height it stated: the root is the
    // client area, so a grid taller than the window is shrunk to it before the tracks divide
    // anything, and the claim is about the division and not about the total.
    let (grid, tall) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[0]), h.model().solved(nodes[2]))
    });
    let half = grid.size.y / 2.0;
    assert!(
        (tall.rect.y0 - grid.rect.y0 - half).abs() < 1.0,
        "the second track must begin at half the grid ({half} DIPs), not below its \
         neighbour's content: it began {} DIPs down",
        tall.rect.y0 - grid.rect.y0
    );
}

/// The first solve applies the class it resolved, including when that class is `Medium`.
///
/// A class matching the solver's own default for an unclassified node produces no transition,
/// so it has to reach the lowered styles on the first layout. Without that the window opens
/// in the arrangement the mount lowered at and corrects itself on the next solve.
#[test]
fn the_first_solve_applies_the_class_it_resolved() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            // 800 against these thresholds is Medium.
            [600.0, 1000.0],
            stack(
                plate()
                    .width(Metric::CardMinW)
                    .height(Metric::CardMinH)
                    .hide_when(windows_scene::WidthClass::Medium),
            ),
        )
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let hidden = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        h.model().solved(nodes[2])
    });
    assert!(
        hidden.size.x < 0.5,
        "the class resolved on the first solve must reach the styles laid out on it"
    );
}

// ── the wash matches what it covers ──────────────────────────────────────────────

/// Every washed control's wash carries the corner radius of the control under it.
///
/// A wash crossfades over the surface it covers, so a radius it does not share paints a
/// square highlight on a round control while hovered.
#[test]
fn a_wash_is_as_round_as_the_control_it_covers() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        // Built inside the loop: the arena clears after each mount, so an element minted before
        // one and used after it names a slot the clear has freed.
        let cases: [(&str, fn() -> View); 3] = [
            ("button", || crate::widget::button("x").erase()),
            ("knob", || {
                crate::widget::knob(0.5_f64, crate::widget::Range::UNIT)
            }),
            ("slider", || {
                crate::widget::slider(
                    0.5_f64,
                    crate::widget::Range::UNIT,
                    crate::widget::SliderStyle::default(),
                )
            }),
        ];
        for (name, view) in cases {
            let mut patch = fixture();
            let _held = mount(view().width(Metric::CardMinW).height(Metric::RowH), root());
            flush(&mut patch);
            let wash = Host::with(|h| {
                h.controls
                    .iter()
                    .next()
                    .and_then(|(_, c)| c.front.wash)
                    .expect("an interactive control mints a wash")
            });
            let radius = patch
                .ops()
                .iter()
                .rev()
                .find_map(|op| match op {
                    Op::Mask {
                        id,
                        mask: windows_scene::Mask::Box { radius },
                        ..
                    } if *id == wash => Some(radius.tl),
                    _ => None,
                })
                .unwrap_or(0.0);
            assert!(
                radius > 0.0,
                "{name}'s wash is square over a rounded control"
            );
        }
    });
}

// ── what automation is told ─────────────────────────────────────────────────────
//
// The seeds below are synthesised from the mount rows themselves, so what a client reads
// is decided here rather than by the seeds a caller hands to `uia::Tree`.

/// Returns the seeds this mount produced, with their names resolved out of the blob.
fn seeds() -> Vec<(crate::widget::UiaRole, String, crate::uia::Value)> {
    let mut out = crate::uia::Seeds::default();
    Host::with(|h| h.uia_seeds(&mut out));
    out.rows
        .iter()
        .map(|seed| {
            let at = seed.name.at as usize;
            let name = String::from_utf16_lossy(&out.blob[at..at + seed.name.len as usize]);
            (seed.role, name, seed.value)
        })
        .collect()
}

/// Returns the published tree this mount would produce.
fn tree(patch: &SinkPatch) -> crate::uia::Tree {
    let mut out = crate::uia::Seeds::default();
    Host::with(|h| h.uia_seeds(&mut out));
    crate::uia::Tree::build(patch.hit_entries(), &out)
}

/// A control takes its name from the text its subtree laid out.
///
/// A button's label is a child element rather than text on the control's own node, so a name
/// read off that node alone is empty.
#[test]
fn a_control_is_named_by_the_text_its_subtree_laid_out() {
    let mut patch = fixture();
    let _button = mount(crate::widget::button("Mute"), root());
    flush(&mut patch);

    let named = seeds();
    assert!(
        named.contains(&(
            crate::widget::UiaRole::Button,
            "Mute".to_owned(),
            crate::uia::Value::None
        )),
        "a button takes its name from its label child: {named:?}"
    );
}

/// Static text is an automation element, and publishes its body as a text document.
///
/// A run with no peer of its own leaves a screen of labels, headings and read-outs reading
/// to a client as an empty window.
#[test]
fn static_text_is_an_element_and_publishes_its_own_body() {
    let mut patch = fixture();
    let _text = mount(crate::widget::text("Output"), root());
    flush(&mut patch);

    let named = seeds();
    assert!(
        named.contains(&(
            crate::widget::UiaRole::Text,
            "Output".to_owned(),
            crate::uia::Value::Text
        )),
        "a run is an element, and its body is a text document: {named:?}"
    );

    let tree = tree(&patch);
    let at = (0..tree.len())
        .find(|&at| {
            tree.col(at)
                .is_some_and(|c| c.role == crate::widget::UiaRole::Text)
        })
        .expect("the run is published");
    assert!(
        tree.patterns(at).has(crate::uia::Patterns::TEXT),
        "and it answers the pattern its body exists for"
    );
}

/// A control with no text of its own takes the name of the run before it.
///
/// A slider carries no text, so without its neighbouring run its published name is empty.
#[test]
fn a_control_with_no_text_takes_the_name_of_the_run_beside_it() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.5_f64);
        let _row = mount(
            stack((
                crate::widget::label("Gain"),
                crate::widget::slider(
                    value,
                    crate::widget::Range::UNIT,
                    crate::widget::SliderStyle::default(),
                )
                .width(Metric::CardMinW),
            )),
            root(),
        );
        flush(&mut patch);

        let tree = tree(&patch);
        let slider = (0..tree.len())
            .find(|&at| {
                tree.col(at)
                    .is_some_and(|c| c.role == crate::widget::UiaRole::Slider)
            })
            .expect("the slider is published");
        let col = tree.col(slider).expect("a column");
        assert_eq!(
            String::from_utf16_lossy(tree.text(col.name)),
            "Gain",
            "the label beside it is its name"
        );
        let label = tree
            .col(col.labelled_by as usize)
            .expect("and it says where that name came from");
        assert_eq!(label.role, crate::widget::UiaRole::Text);
    });
}

/// A capitalised run draws in capitals and announces what the author wrote.
///
/// Casing is a typographic treatment of the label rung, not a rename. A reader given the
/// drawn form says a heading one letter at a time.
#[test]
fn a_capitalised_run_announces_the_authors_casing() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.5_f64);
        let _row = mount(
            stack((
                crate::widget::styled_text(
                    "Gain adjust",
                    crate::widget::TextStyle {
                        caps: true,
                        ..crate::widget::TextStyle::new(TypeRole::Label)
                    },
                ),
                crate::widget::slider(
                    value,
                    crate::widget::Range::UNIT,
                    crate::widget::SliderStyle::default(),
                )
                .width(Metric::CardMinW),
            )),
            root(),
        );
        flush(&mut patch);

        // The shaped form: what the run was laid out from, which is the only place the drawn
        // casing can be read back without rasterizing.
        let shaped = Host::with(|_| {
            crate::build::text::with(|table| {
                table
                    .entries
                    .iter()
                    .map(|(_, entry)| entry.shaped_str().to_owned())
                    .find(|s| s.eq_ignore_ascii_case("gain adjust"))
            })
        })
        .expect("the label is in the table");
        assert_eq!(shaped, "GAIN ADJUST", "the recipe requests capitals");

        let tree = tree(&patch);
        let slider = (0..tree.len())
            .find(|&at| {
                tree.col(at)
                    .is_some_and(|c| c.role == crate::widget::UiaRole::Slider)
            })
            .expect("the slider is published");
        let col = tree.col(slider).expect("a column");
        assert_eq!(
            String::from_utf16_lossy(tree.text(col.name)),
            "Gain adjust",
            "and announces the string the author wrote"
        );
    });
}

/// A control with its own text keeps it, and one whose predecessor is not a run takes none.
///
/// The neighbour rule reaches one element back, so it cannot relabel a named control or
/// claim a heading two controls up.
#[test]
fn a_control_that_has_a_name_keeps_it_and_one_with_no_run_before_it_gets_none() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let value = crate::signal::Cell::new(0.5_f64);
        let _row = mount(
            stack((
                crate::widget::label("Gain"),
                crate::widget::button("Reset"),
                crate::widget::slider(
                    value,
                    crate::widget::Range::UNIT,
                    crate::widget::SliderStyle::default(),
                )
                .width(Metric::CardMinW),
            )),
            root(),
        );
        flush(&mut patch);

        let tree = tree(&patch);
        let role_of = |want| {
            (0..tree.len())
                .find(|&at| tree.col(at).is_some_and(|c| c.role == want))
                .and_then(|at| tree.col(at))
                .copied()
        };
        let button = role_of(crate::widget::UiaRole::Button).expect("the button");
        assert_eq!(
            String::from_utf16_lossy(tree.text(button.name)),
            "Reset",
            "a control with its own text is not relabelled by its neighbour"
        );
        let slider = role_of(crate::widget::UiaRole::Slider).expect("the slider");
        assert!(
            slider.name.is_empty(),
            "and one whose neighbour is a button, not a run, takes nothing"
        );
    });
}

/// A label that re-reads marks the published tree stale.
///
/// A name is a copy in the published blob rather than a live property, so a changed string
/// raises no event and the tree holds the old one until the next publish.
#[test]
fn a_label_that_changes_marks_the_accessible_tree_stale() {
    let mut patch = fixture();
    let caption = crate::signal::Cell::new("Off".to_owned());
    let _text = mount(
        crate::widget::text(crate::widget::reactive(move |out| {
            caption.with(|s| out.push_str(s));
        })),
        root(),
    );
    flush(&mut patch);
    Host::with(|h| h.uia_published());
    assert!(!Host::with(|h| h.uia_stale()), "nothing has moved yet");

    caption.set("On".to_owned());
    crate::signal::flush();
    assert!(
        Host::with(|h| h.uia_stale()),
        "a changed string is a name the published tree is now wrong about"
    );
    let named = seeds();
    assert!(
        named.iter().any(|(_, name, _)| name == "On"),
        "and the next publish carries the new one: {named:?}"
    );
}

/// A readout renders its value, and follows it.
///
/// [`shown`](crate::widget::shown) formats through `Display` straight into the run's buffer,
/// so no `String` sits between the value and the glyphs.
#[test]
fn a_shown_readout_renders_its_value_and_follows_it() {
    let mut patch = fixture();
    let count = crate::signal::Cell::new(7_usize);
    let _text = mount(
        crate::widget::mono(crate::widget::shown(move || count.get())),
        root(),
    );
    flush(&mut patch);
    assert!(
        seeds().iter().any(|(_, name, _)| name == "7"),
        "the readout must show the value it was given: {:?}",
        seeds()
    );

    count.set(12);
    crate::signal::flush();
    assert!(
        seeds().iter().any(|(_, name, _)| name == "12"),
        "and follow it: {:?}",
        seeds()
    );
}

/// A readout whose value moves but whose formatted text does not allocates nothing.
///
/// `-6.031` and `-6.028` both format to `-6.0 dB`. The table declines to reshape a string
/// that did not move, and discovering that it did not move costs no allocation: the source
/// writes in place rather than answering with a `String`. A dragged control takes this path
/// at display rate.
#[test]
fn a_readout_whose_text_does_not_move_allocates_nothing() {
    use core::fmt::Write;

    let mut patch = fixture();
    let level = crate::signal::Cell::new(-6.031_f64);
    let _text = mount(
        crate::widget::mono(crate::widget::reactive(move |out| {
            let _ = write!(out, "{:.1} dB", level.get());
        })),
        root(),
    );
    flush(&mut patch);
    // One warm-up: the scratch buffer and the scheduler's own queues each reach their
    // high-water mark once, and the count below is of the steady state after that.
    level.set(-6.030);
    crate::signal::flush();

    let before = crate::counting::allocations();
    level.set(-6.028);
    crate::signal::flush();
    let during = crate::counting::allocations() - before;

    assert_eq!(
        during, 0,
        "a readout settling on the same text allocated {during} times"
    );
}

/// A run bound to a memo follows it, as a run bound to a cell does.
///
/// A memo is minted once and never rebuilt, so a run's binding tracks the memo rather than
/// the cell underneath it.
#[test]
fn a_run_bound_to_a_memo_follows_it() {
    let mut patch = fixture();
    let selection = crate::signal::Cell::new(None::<u32>);
    let selected = crate::signal::Memo::new(move || selection.get());
    let runs = std::rc::Rc::new(std::cell::Cell::new(0_u32));
    let counter = std::rc::Rc::clone(&runs);
    let _held = mount(
        stack(crate::widget::text(crate::widget::reactive(move |out| {
            counter.set(counter.get() + 1);
            selected.with(|s| {
                out.push_str(match s {
                    Some(_) => "a much longer line of text than the other one",
                    None => "x",
                });
            });
        })))
        .width(Len::Pct(1.0)),
        root(),
    );
    crate::signal::flush();
    flush(&mut patch);
    let run = Host::with(|h| {
        h.mounts
            .iter()
            .map(|(_, m)| m.node)
            .nth(1)
            .expect("the run")
    });
    let absent = Host::with(|h| h.model().solved(run).size.x);

    selection.set(Some(1));
    crate::signal::flush();
    flush(&mut patch);
    let present = Host::with(|h| h.model().solved(run).size.x);

    assert!(
        present > absent,
        "the run measured {absent} DIPs before the memo moved and {present} after, so the \
         memo's change never reached it (the binding ran {} times)",
        runs.get()
    );
}

/// A hidden subtree lays out as hidden all the way down, including a measurable leaf.
///
/// Taffy descends into a hidden subtree with `RunMode::PerformHiddenLayout`, where a measure
/// function may not be called, so the hidden decision follows the run mode rather than each
/// node's own display. A text run inside is what reaches the leaf path: a hidden node whose
/// descendants are all boxes never reaches a measure at all.
#[test]
fn a_hidden_subtree_does_not_measure_its_leaves() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            [900.0, 1000.0],
            stack((
                crate::widget::caption("visible"),
                stack(crate::widget::caption("hidden"))
                    .hide_when(windows_scene::WidthClass::Narrow),
            )),
        )
        .width(Len::Pct(1.0)),
        root(),
    );
    // The assertion is that this returns at all.
    flush(&mut patch);
}

// ── what a structural adapter contributes to its parent's layout ─────────────────
//
// An adapter parents its rows and arms straight to the container it was passed to, and adds
// only a zero-size anchor for identity. A group of its own would impose that group's style on
// everything below it: `Preset::Bare` is `Style::DEFAULT`, a content-sized flex row, so rows
// would march across whatever their container was and an arm could not fill its box.

/// A keyed list is laid out by the container it was passed to.
///
/// The container here is a column, so its rows share a left edge and descend.
#[test]
fn a_keyed_list_lays_out_under_its_container() {
    let mut patch = fixture();
    let _list = mount(
        stack(each(
            || (0_u32..3).collect(),
            |item| item,
            |_| plate().height(Metric::CardMinH),
        ))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    // The stack, its adapter's anchor, then the three rows in mount order.
    let rows = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        [
            h.model().solved(nodes[2]),
            h.model().solved(nodes[3]),
            h.model().solved(nodes[4]),
        ]
    });
    for pair in rows.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(
            (a.rect.x0 - b.rect.x0).abs() < 1.0,
            "a column's rows share a left edge: {} then {}",
            a.rect.x0,
            b.rect.x0
        );
        assert!(
            b.rect.y0 > a.rect.y0,
            "and they descend: {} then {}",
            a.rect.y0,
            b.rect.y0
        );
    }
}

/// An adapter's anchor occupies no space in its parent's layout.
///
/// The anchor is in the parent's child list to carry identity, so a list of two rows measures
/// as exactly two rows and leaves no gap the author cannot remove.
#[test]
fn an_adapters_anchor_takes_no_space() {
    let mut patch = fixture();
    let _list = mount(
        stack(each(
            || (0_u32..2).collect(),
            |item| item,
            |_| plate().height(Metric::CardMinH),
        ))
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (anchor, first, second) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (
            h.model().solved(nodes[1]),
            h.model().solved(nodes[2]),
            h.model().solved(nodes[3]),
        )
    });
    assert_eq!(
        anchor.size,
        windows_numerics::Vector2 { x: 0.0, y: 0.0 },
        "the anchor is hidden, so it has no size at all"
    );
    assert!(
        (second.rect.y0 - first.rect.y1).abs() < 1.0,
        "at zero gap the rows abut: {} then {}",
        first.rect.y1,
        second.rect.y0
    );
}

/// A `switch` arm fills the box it was placed in.
///
/// The arm is a child of the container, so a `.grow()` on the arm reaches that container's
/// own sizing rather than stopping at an intervening node with `flex_grow: 0`.
#[test]
fn a_switch_arm_fills_the_box_it_was_placed_in() {
    let mut patch = fixture();
    let _held = mount(
        stack(switch(|| 0_u8, |_| plate().grow()))
            .height(Len::Pct(1.0))
            .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (container, arm) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[0]), h.model().solved(nodes[2]))
    });
    assert!(
        (arm.size.y - container.size.y).abs() < 1.0,
        "the arm must take the container's height: {} of {}",
        arm.size.y,
        container.size.y
    );
}

/// Two adjacent branches keep their declared order across both being empty.
///
/// The anchor gives each adapter its own predecessor. Without one, two arms absent at mount
/// would share a predecessor, and whichever filled second would be placed at the same
/// position as the first and land above it. Here the second branch fills first.
#[test]
fn two_adjacent_branches_keep_their_order_across_being_empty() {
    let mut patch = fixture();
    let (first, second) = (
        crate::signal::Cell::new(false),
        crate::signal::Cell::new(false),
    );
    let _held = mount(
        stack((
            when(first, || plate().height(Metric::CardMinH)),
            when(second, || plate().height(Metric::CardMinH)),
        ))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    // The lower one appears first, so nothing about the order can come from mount order.
    second.set(true);
    crate::signal::flush();
    flush(&mut patch);
    first.set(true);
    crate::signal::flush();
    flush(&mut patch);

    let (lower, upper) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        // The stack, two anchors, then `second`'s arm and `first`'s arm in the order they
        // were filled.
        (h.model().solved(nodes[3]), h.model().solved(nodes[4]))
    });
    assert!(
        upper.rect.y0 < lower.rect.y0,
        "the branch written first must lay out above the one written second, whichever \
         filled first: {} against {}",
        upper.rect.y0,
        lower.rect.y0
    );
}

/// A wrapping run breaks against the track it was placed in, and grows down when it does.
///
/// The width alone settles nothing: a run laid out as a single line still has its node
/// clamped by the track, and draws its glyphs straight through the column's edge. The height
/// is what separates the two, because a wrapping run owns a sprite per line and `Preset::Text`
/// stacks those down rather than across.
#[test]
fn a_wrapping_run_breaks_against_its_column() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            stack(crate::widget::caption(
                "Latency, initialization time and total CPU belong here — the figures the \
                 config format cannot tell you. They are left blank rather than invented.",
            )),
        ))
        .cols([Track::Fr(1.0), Track::Fixed(Len::Pct(0.25))])
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (column, run) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[2]), h.model().solved(nodes[3]))
    });
    // A quarter of the fixture's 800-DIP window.
    assert!(
        column.size.x <= 200.0 + 1.0,
        "the prose column measured {} DIPs against a 200-DIP track, so it wrapped against \
         nothing and drew through the edge",
        column.size.x
    );
    // That sentence is far longer than a 200-DIP line holds, so it cannot be one.
    assert!(
        run.size.y > 2.0 * run_line_height(),
        "the run is {} DIPs tall — a paragraph laid out as a single line",
        run.size.y
    );
}

#[test]
fn source_text_preserves_lines_wraps_and_settles_after_replacement() {
    let mut patch = fixture();
    let value = crate::signal::Cell::new("[[block]]\nq = 4.125".to_string());
    let _held = mount(
        stack(crate::widget::code(crate::widget::reactive(move |out| {
            value.with(|s| out.push_str(s));
        })))
        .width(Len::Pct(0.25))
        .min_width(Len::Zero),
        root(),
    );
    let size = || {
        Host::with(|h| {
            let node = h.mounts.iter().nth(1).unwrap().1.node;
            h.model().solved(node).size
        })
    };
    flush(&mut patch);
    let lines = size();
    value.set("[[block]]".into());
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        lines.y > size().y,
        "source newlines must create distinct baselines"
    );
    value.set(format!(
        "original_path = \"C:/{}\"",
        "long-directory/".repeat(16)
    ));
    crate::signal::flush();
    flush(&mut patch);
    assert!(size().x <= 201.0);
    assert!(
        size().y > lines.y,
        "long source must wrap inside its column"
    );
    patch.clear();
    flush(&mut patch);
    assert!(
        patch.ops().is_empty(),
        "unchanged source must emit no retained work"
    );
}

/// Returns one caption line's height at the fixture's scope, read from the palette's ramp.
///
/// Reading it rather than writing it down keeps the assertions that use it moving with the
/// type ramp instead of pinning it.
fn run_line_height() -> f32 {
    Host::with(|h| crate::role::typography(crate::role::TypeRole::Caption, h.root_scope).size)
}

/// A wrapping run inside a `switch` arm breaks against its column.
///
/// An arm is a child of the container, so a caption reaching layout through an adapter is
/// measured against the same track as one placed directly.
#[test]
fn a_wrapping_run_inside_an_arm_breaks_against_its_column() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            stack(switch(
                || 0_u8,
                |_| {
                    stack(crate::widget::caption(
                        "Latency, initialization time and total CPU belong here — the figures \
                         the config format cannot tell you. They are left blank rather than \
                         invented.",
                    ))
                },
            )),
        ))
        .cols([Track::Fr(1.0), Track::Fixed(Len::Pct(0.25))])
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let (column, arm) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[2]), h.model().solved(nodes[4]))
    });
    assert!(
        column.size.x <= 200.0 + 1.0,
        "the column measured {} DIPs against a 200-DIP track",
        column.size.x
    );
    assert!(
        arm.size.x <= 200.0 + 1.0,
        "the arm measured {} DIPs inside a 200-DIP column, so its prose drew through the edge",
        arm.size.x
    );
}

/// A wrapping run breaks against the room its containers' padding leaves it.
///
/// A padded section inside a padded surface puts two insets between the prose and the column,
/// and measuring against the column instead overflows by exactly those insets, which shows up
/// as text running off the window's edge.
#[test]
fn a_wrapping_run_breaks_inside_its_containers_padding() {
    use crate::layout::Track;
    let mut patch = fixture();
    let _held = mount(
        crate::layout::grid((
            plate().height(Metric::CardMinH),
            stack(
                stack(crate::widget::caption(
                    "Latency, initialization time and total CPU belong here — the figures the \
                 config format cannot tell you. They are left blank rather than invented.",
                ))
                .padding(Len::Metric(Metric::SpaceMd)),
            )
            .padding(Len::Metric(Metric::SpaceLg)),
        ))
        .cols([Track::Fr(1.0), Track::Fixed(Len::Pct(0.25))])
        .gap(Len::Zero)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let run = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        h.model()
            .solved(*nodes.last().expect("the run is the deepest node"))
    });
    let space = Host::with(|h| {
        (
            crate::role::metric(Metric::SpaceLg, h.root_scope),
            crate::role::metric(Metric::SpaceMd, h.root_scope),
        )
    });
    let room = 200.0 - 2.0 * space.0 - 2.0 * space.1;
    assert!(
        run.size.x <= room + 1.0,
        "the run measured {} DIPs against the {room} its two paddings left it",
        run.size.x
    );
}

/// A run that can break answers taffy's two intrinsic probes differently.
///
/// `MeasureIn::available` carries `MinContent` and `MaxContent` apart rather than flattening
/// both to indefinite, so a paragraph does not answer its one-line width to min-content. This
/// is asserted at the measure seam, and the widths come from the engine rather than being
/// written down, so it moves with the type ramp.
///
/// The single-line run is the control: it has no break opportunity, so its narrowest width is
/// its widest and both probes give one answer.
#[test]
fn the_two_intrinsic_probes_differ_for_a_run_that_can_break() {
    let mut patch = fixture();
    let _held = mount(
        stack((
            crate::widget::caption(
                "Latency, initialization time and total CPU belong here — the figures the \
                 config format cannot tell you.",
            ),
            crate::widget::label("Bypassed"),
        )),
        root(),
    );
    flush(&mut patch);

    let keys: Vec<_> = Host::with(|h| h.mounts.iter().filter_map(|(_, m)| m.text).collect());
    assert_eq!(keys.len(), 2, "the two runs registered");
    let probe = |key, avail| {
        text::measure(windows_scene::MeasureIn {
            key,
            class: crate::role::WidthClass::Wide,
            known: (None, None),
            available: (avail, windows_scene::Avail::MaxContent),
        })
    };
    use windows_scene::Avail::{MaxContent, MinContent};

    let (prose_min, prose_max) = (probe(keys[0], MinContent), probe(keys[0], MaxContent));
    assert!(
        prose_min.x < prose_max.x,
        "the paragraph answered {} DIPs to both probes, so its min-content is its whole line",
        prose_min.x
    );
    // Narrower and taller: a run measured at its longest word occupies several lines, and a
    // width reported without the matching height is a box that clips its own text.
    assert!(
        prose_min.y > prose_max.y,
        "the paragraph reported one line's height ({}) at min-content",
        prose_min.y
    );

    let (label_min, label_max) = (probe(keys[1], MinContent), probe(keys[1], MaxContent));
    assert_eq!(
        label_min, label_max,
        "a single-line run has no break opportunity, so both probes are one answer"
    );
}

/// A scroll container inside a hidden subtree defers its tracker until it is shown, and
/// creates one then.
///
/// `hide_if` and `when` are `Display::None` rather than an unmount, so the subtree stays
/// mounted and is solved at zero. A `VisualInteractionSource` takes its hit region from the
/// viewport's size at the moment it is created, so one created there hit-tests nothing;
/// `Scene::tracker` rejects a zero-size viewport rather than accepting it.
#[test]
fn a_hidden_scroll_container_defers_its_tracker_until_it_is_shown() {
    let hidden = crate::signal::Cell::new(true);
    let mut patch = fixture();
    let _held = mount(
        crate::layout::scroll(plate().height(Metric::CardMinH))
            .height(Metric::CardMinH)
            .hide_if(move || hidden.get()),
        root(),
    );
    flush(&mut patch);

    let creates = |patch: &SinkPatch| {
        patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Tracker {
                        op: windows_scene::TrackerOp::Create { .. },
                        ..
                    }
                )
            })
            .count()
    };
    assert_eq!(
        creates(&patch),
        0,
        "a tracker was created against a viewport laid out at zero"
    );

    let content = Host::with(|h| h.scrolls.iter().next().unwrap().1.content);
    let content_binding = |patch: &SinkPatch| {
        patch.ops().iter().position(|op| {
            matches!(
                op,
                Op::Bind {
                    id,
                    prop: windows_scene::Prop::OffsetY,
                    bind: windows_scene::Bind::Track { .. },
                } if *id == content
            )
        })
    };
    assert!(
        content_binding(&patch).is_none(),
        "a hidden scroll bound its content before its tracker existed"
    );

    patch.clear();
    hidden.set(false);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(
        creates(&patch),
        1,
        "the viewport has a box now and its tracker was never created"
    );
    let created = patch
        .ops()
        .iter()
        .position(|op| {
            matches!(
                op,
                Op::Tracker {
                    op: windows_scene::TrackerOp::Create { .. },
                    ..
                }
            )
        })
        .unwrap();
    assert!(
        content_binding(&patch).expect("revealing the scroll did not bind its content") > created,
        "the content binding preceded tracker creation and would be dropped by the scene"
    );

    patch.clear();
    flush(&mut patch);
    assert_eq!(creates(&patch), 0, "idle recreated the tracker");
    assert!(
        content_binding(&patch).is_none(),
        "idle rebound the content"
    );
}

// ── reading back the boxes the solve produced ────────────────────────────────────

/// A probe reports its node's solved box, as a signal.
///
/// A gutter drawn beside independently-sized rows meets each row at its resolved centre, and
/// no container holds both halves. The rows here are given different heights, so a probe
/// reporting a uniform stride rather than each node's own box fails.
#[test]
fn a_probe_reports_where_the_solve_put_its_node() {
    let (first, second) = (crate::layout::probe(), crate::layout::probe());
    let mut patch = fixture();
    let _held = mount(
        stack((
            plate().height(Metric::RowH).probed(first),
            plate().height(Metric::CardMinH).probed(second),
        ))
        .gap(Len::Zero),
        root(),
    );
    flush(&mut patch);

    let (a, b) = (first.get(), second.get());
    assert!(a.size.y > 0.0, "the first row was never reported");
    assert_eq!(
        b.rect.y0, a.rect.y1,
        "the second row does not begin where the first ended, so these are not the boxes \
         the solve produced"
    );
    assert!(
        b.size.y > a.size.y,
        "both rows reported {} DIPs — a probe reporting a uniform stride is no use to the \
         thing it exists for",
        a.size.y
    );
    assert_eq!(
        a.size.x, b.size.x,
        "the rows stretch to one column, so their widths agree"
    );
}

/// A probe writes only when its node's box moves.
///
/// The equality gate keeps a probe off the per-frame path: a solve that moves nothing wakes
/// nothing derived from it. The count is a `Memo`'s recomputes, which is what a consumer of
/// the probe pays.
#[test]
fn a_probe_publishes_only_when_its_node_moves() {
    let tall = crate::signal::Cell::new(false);
    let where_ = crate::layout::probe();
    let mut patch = fixture();
    let _held = mount(
        stack(
            plate()
                .height(Metric::RowH)
                .no_shrink()
                .when(move || !tall.get()),
        )
        .probed(where_)
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let counted = std::rc::Rc::new(std::cell::Cell::new(0_u32));
    let seen = crate::signal::Memo::new({
        let counted = std::rc::Rc::clone(&counted);
        move || {
            counted.set(counted.get() + 1);
            where_.get().size.y
        }
    });
    seen.get();
    let after_first = counted.get();

    // A flush that changes nothing must not disturb it.
    flush(&mut patch);
    crate::signal::flush();
    seen.get();
    assert_eq!(
        counted.get(),
        after_first,
        "a solve that moved nothing still published a box"
    );

    // A flush that does change the box must.
    tall.set(true);
    crate::signal::flush();
    flush(&mut patch);
    crate::signal::flush();
    seen.get();
    assert!(
        counted.get() > after_first,
        "the node's height changed and the probe never said so"
    );
}

#[test]
fn local_geometry_tracks_local_inputs_and_releases_its_shared_resource() {
    use crate::signal::{Cell, Owner};
    use windows_scene::{PathVerb, ResOp};
    let mut patch = fixture();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = calls.clone();
    let (owner, (lead, data, mount)) = Owner::scope(|| {
        let lead = Cell::new(false);
        let data = Cell::new(1.0);
        let bounds = crate::layout::probe();
        let id = local_geometry(bounds, 2, move |verbs, size, scope| {
            assert_eq!(scope.elevation, Elevation::Raised);
            observed.set(observed.get() + 1);
            verbs.extend([
                PathVerb::Move {
                    to: Vector2::default(),
                    filled: false,
                },
                PathVerb::Line(Vector2 {
                    x: size.x,
                    y: data.get(),
                }),
            ]);
        });
        let view = stack((
            plate()
                .height(Metric::RowH)
                .no_shrink()
                .when(move || lead.get()),
            stack((crate::widget::path(id), crate::widget::path(id)))
                .probed(bounds)
                .elevate(Elevation::Raised)
                .height(Metric::RowH)
                .width(Len::Pct(1.0)),
        ))
        .width(Len::Pct(1.0));
        (lead, data, mount(view, root()))
    });
    let settle = |patch: &mut SinkPatch| {
        for _ in 0..4 {
            crate::signal::flush();
            flush(patch);
        }
    };
    settle(&mut patch);
    assert_eq!(
        calls.get(),
        1,
        "both sprites share one geometry computation"
    );
    lead.set(true);
    settle(&mut patch);
    assert_eq!(calls.get(), 1, "parent movement is not a local shape input");
    Host::with(|h| h.set_window(Vector2 { x: 400.0, y: 600.0 }));
    settle(&mut patch);
    assert_eq!(calls.get(), 2);
    data.set(2.0);
    settle(&mut patch);
    assert_eq!(calls.get(), 3);
    drop(mount);
    patch.clear();
    drop(owner);
    flush(&mut patch);
    assert_eq!(
        patch
            .ops()
            .iter()
            .filter(|op| matches!(
                op,
                Op::Res {
                    op: ResOp::Drop,
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(calls.get(), 3);
}

/// A probe attached inside a subtree that unmounts is released with it.
///
/// The cell dies with the scope that made it and the row with the mount walk, so the publish
/// tolerates either order. Writing a disposed cell panics, so the flush after the unmount
/// returning at all is half the assertion.
#[test]
fn a_probe_survives_its_subtree_unmounting() {
    let shown = crate::signal::Cell::new(true);
    let where_ = crate::layout::probe();
    let mut patch = fixture();
    let _held = mount(
        stack(switch(
            move || shown.get(),
            move |on| {
                if *on {
                    plate().height(Metric::RowH).probed(where_).erase()
                } else {
                    crate::widget::caption("gone").erase()
                }
            },
        ))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);
    assert!(
        where_.get().size.y > 0.0,
        "the probed node was never solved"
    );

    shown.set(false);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(
        Host::with(|h| h.probes.iter().count()),
        0,
        "the probe row outlived the subtree that declared it"
    );
}

#[test]
fn navigation_releases_probes_owned_by_nested_branches() {
    let shown = crate::signal::Cell::new(true);
    let mut patch = fixture();
    let _held = mount(
        stack(when(
            move || shown.get(),
            || {
                stack(when(
                    || true,
                    || {
                        let location = crate::layout::probe();
                        plate().height(Metric::RowH).probed(location)
                    },
                ))
            },
        ))
        .width(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);
    assert_eq!(Host::with(|h| h.probes.iter().count()), 1);

    shown.set(false);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(Host::with(|h| h.probes.iter().count()), 0);

    shown.set(true);
    crate::signal::flush();
    flush(&mut patch);
    assert_eq!(Host::with(|h| h.probes.iter().count()), 1);
}

#[test]
fn disposing_the_application_owner_releases_scalar_controls_before_final_flush() {
    let mut patch = fixture();
    let (owner, held) = crate::signal::Owner::scope(|| {
        mount(
            stack(when(
                || true,
                || {
                    let value = crate::signal::Cell::new(true);
                    crate::widget::toggle(value)
                },
            )),
            root(),
        )
    });
    flush(&mut patch);
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        Host::with(|h| h
            .controls
            .iter()
            .filter(|(_, c)| c.front.thumb.is_some())
            .count())
            > 0
    );
    patch.clear();
    drop(owner);
    drop(held);
    assert_eq!(
        Host::with(|h| h
            .controls
            .iter()
            .filter(|(_, c)| c.front.thumb.is_some())
            .count()),
        0
    );
    flush(&mut patch);
    assert!(
        !patch.ops().iter().any(|op| matches!(op, Op::Bind { .. })),
        "the final flush must not bind destroyed controls"
    );
}

/// A probed path revealed by `when` is given its container's whole inner width.
///
/// The chain row's shape: an accent edge beside a grown column, whose body is mounted when
/// the row opens rather than being there all along. Both halves a figure is drawn from are
/// asserted — the width a `Len::Pct(1.0)` leaf resolves against, and the box the probe
/// reports it at — so a figure that renders short of its box is not this.
#[test]
fn a_probed_path_revealed_by_when_fills_the_column_it_opens_in() {
    let (column, figure) = (crate::layout::probe(), crate::layout::probe());
    let open = crate::signal::Cell::new(false);
    let mut patch = fixture();
    let _held = mount(
        crate::widget::card()
            .row((
                plate().width(Len::Times(Metric::HairlineW, 3.0)),
                stack((
                    crate::widget::label("header"),
                    when(open, move || {
                        crate::widget::path(super::geometry(&[]))
                            .stroke(crate::role::DataRole(0), Metric::HairlineW)
                            .probed(figure)
                            .width(Len::Pct(1.0))
                            .height(Len::Times(Metric::RowH, 2.0))
                            .erase()
                    }),
                ))
                .probed(column)
                .padding(Metric::SpaceSm)
                .grow(),
            ))
            .padding(Len::Zero)
            .gap(Len::Zero)
            .align(crate::layout::Align::Stretch)
            .min_height(Metric::RowH),
        root(),
    );
    flush(&mut patch);
    assert_eq!(
        figure.get().size.x,
        0.0,
        "the body is shut, so its figure has no box at all"
    );

    open.set(true);
    crate::signal::flush();
    flush(&mut patch);

    let padding = 2.0 * crate::role::metric(Metric::SpaceSm, Host::with(|h| h.root_scope));
    let (column, figure) = (column.get().size.x, figure.get().size.x);
    assert!(column > padding, "the column solved to {column}");
    assert!(
        (figure - (column - padding)).abs() < 0.5,
        "the figure was given {figure} DIPs of a {column}-DIP column padded by {padding}"
    );
}

/// A sprite's strength scales the role's own alpha rather than replacing it.
///
/// A hairline resolves to a wash already. Replacing its alpha with the strength would make
/// every border in the interface opaque at full strength, which is a plausible-looking
/// regression: the ring stays where it is and only gets heavier.
#[test]
fn a_sprites_strength_scales_the_roles_own_alpha() {
    let mut patch = fixture();
    let full = El::<Any>::seed(crate::layout::Preset::Bare).sprite(
        MaskSeed::Box { radius: None },
        Role::Stroke(Stroke::Subtle),
        Part::Fill,
    );
    let half = El::<Any>::seed(crate::layout::Preset::Bare).sprite_at(
        MaskSeed::Box { radius: None },
        Role::Stroke(Stroke::Subtle),
        Part::Fill,
        0.5,
    );
    let _mount = mount(stack((full, half)), root());
    flush(&mut patch);

    let alphas: Vec<f32> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Paint {
                paint: Paint::Solid(light),
                ..
            } => Some(light.a),
            _ => None,
        })
        .collect();
    assert_eq!(alphas.len(), 2, "one sprite each");
    let subtle = crate::role::resolve(
        Role::Stroke(Stroke::Subtle),
        Scope::root(
            crate::role::tests::palette(),
            AccentId(0),
            Density::Comfortable,
        )
        .for_paint(),
    )
    .a;
    assert!(
        (alphas[0] - subtle).abs() < 1e-6,
        "full strength must leave the role's wash where it is, got {}",
        alphas[0]
    );
    assert!(
        (alphas[1] - subtle * 0.5).abs() < 1e-6,
        "a strength scales that wash, got {}",
        alphas[1]
    );
}

/// A ghost control that can be selected mints the sprite selection paints into.
///
/// Its resting row carries no fill, so counting sprites off that row alone leaves selection
/// with nowhere to go: the control looks identical selected and not, which is what a nav rail
/// and a segmented picker are entirely made of.
#[test]
fn a_selectable_ghost_mints_the_fill_its_selected_state_needs() {
    let mut patch = fixture();
    let paints = |patch: &SinkPatch| {
        patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Paint {
                        paint: Paint::Solid(_),
                        ..
                    }
                )
            })
            .count()
    };

    let plain = mount(stack(crate::widget::button("plain").ghost()), root());
    flush(&mut patch);
    let without = paints(&patch);
    patch.clear();
    drop(plain);

    let picks = mount(
        stack(crate::widget::button("picks").ghost().selected(|| false)),
        root(),
    );
    flush(&mut patch);
    let with = paints(&patch);
    drop(picks);

    // Exactly one more sprite: the fill the selected row supplies and the ghost row does not.
    // A ghost that never declares selection must not pay for a sprite it can never show.
    assert_eq!(
        with,
        without + 1,
        "declaring selection costs one sprite, and declaring none costs zero"
    );
}

/// A scroll container clips its pixels, not only its hits.
///
/// The solve marks an overflow container `bounded`, and the hit array reads that flag. Until
/// the placement pass emitted a clip alongside it, nothing did on the drawing side: a long
/// list bounded what the pointer could reach and painted straight over the bands below it.
#[test]
fn a_scroll_container_clips_what_it_draws() {
    let mut patch = fixture();
    let _mount = mount(
        crate::layout::scroll(stack((plate(), plate(), plate()))).height(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);

    let clipped = patch.ops().iter().any(|op| {
        matches!(
            op,
            Op::Clip {
                clip: windows_scene::Clip::Rect { r, b, .. },
                ..
            } if *r > 0.0 && *b > 0.0
        )
    });
    assert!(
        clipped,
        "the viewport must publish a clip at its own box, or its content draws outside it"
    );
}

/// A toggle's knob has extent, and its track is long enough to round as a stadium.
///
/// A bare node has no intrinsic size, so a knob that states none solves to nothing: it is
/// invisible, and the travel `along` computes — the track's extent less the knob's — is the
/// whole track. The existing travel assertion passes either way, which is how a switch
/// shipped as one flat blob.
#[test]
fn a_toggles_knob_has_extent_inside_its_track() {
    let mut patch = fixture();
    let on = crate::signal::Cell::new(true);
    let _toggle = mount(crate::widget::toggle(on), root());
    flush(&mut patch);

    let scope = Scope::root(
        crate::role::tests::palette(),
        AccentId(0),
        Density::Comfortable,
    );
    let row = crate::role::metric(Metric::RowH, scope);
    let travel = Host::with(|h| {
        h.controls
            .iter()
            .next()
            .map_or(0.0, |(_, c)| c.front.travel)
    });
    assert!(
        travel > 0.0,
        "a knob in a sized track has room to move, got {travel}"
    );
    // The track's own width, which is what the travel would equal if the knob had no extent.
    let width = row * 1.7;
    assert!(
        travel < width - 1.0,
        "the knob must take room out of its own travel: travel {travel} against a {width} track"
    );
    // The contract a pill radius carries: a control one row tall must be fully rounded by it.
    // Above half a row the platform's per-axis cap differs between the axes and the corners
    // meet in the middle, which renders the switch as a lens.
    assert!(
        crate::role::metric(Metric::RadiusPill, scope) * 2.0 <= row + 1e-3,
        "a pill radius above half a row cannot round a row-tall track as a stadium"
    );
}

/// A toggle that is on leaves its knob at the end of its travel, and nothing writes over it.
///
/// The knob's position is an `OffsetX` the app thread binds from the value; the placement
/// pass binds a whole `Offset` for every node the solve moved. Both land on the same channel,
/// so the one that runs last decides where the knob sits.
#[test]
fn a_toggle_that_is_on_leaves_its_knob_at_the_end_of_its_travel() {
    let mut patch = fixture();
    let on = crate::signal::Cell::new(true);
    let _toggle = mount(crate::widget::toggle(on), root());
    flush(&mut patch);

    let (rest, travel, thumb, node) = Host::with(|h| {
        h.controls
            .iter()
            .next()
            .map_or((0.0, 0.0, None, windows_scene::NodeId::NONE), |(_, c)| {
                (c.front.rest, c.front.travel, c.front.thumb, c.node)
            })
    });
    let thumb = thumb.expect("a toggle mints a knob");
    assert!(travel > 0.0, "the knob has room to move");
    assert!(rest > 0.0, "the knob is inset from the track's own edge");

    // The last write to the knob's own x, in patch order.
    let mut last = None;
    for op in patch.ops() {
        match op {
            Op::Bind {
                id,
                prop: windows_scene::Prop::Offset,
                bind: windows_scene::Bind::Set(windows_scene::Value::Vec2(v)),
            } if *id == thumb => last = Some(v.x),
            Op::Bind {
                id,
                prop: windows_scene::Prop::OffsetX,
                bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(x)),
            } if *id == thumb => last = Some(*x),
            _ => {}
        }
    }
    assert_eq!(
        last,
        Some(rest),
        "layout publishes the resting box; scene adoption places the scalar part"
    );
    // The gap the knob leaves at the far end is the one it rests at, so the two ends of the
    // switch look the same and neither shows the knob overhanging the track.
    let (track, knob) = Host::with(|h| (h.model().solved(node).size, h.model().solved(thumb).size));
    assert!(
        ((track.x - (rest + travel) - knob.x) - rest).abs() <= 0.5,
        "the knob is inset by {rest} at the near end and {} at the far one",
        track.x - (rest + travel) - knob.x
    );
}

/// Flipping a toggle springs its knob rather than putting it at the far end in one frame.
///
/// The knob's channel is the only thing about a switch that moves: the track's fill is a
/// discrete paint swap at event rate, so a knob that snapped would leave the control with no
/// motion at all.
#[test]
fn flipping_a_toggle_publishes_its_value_to_the_scene() {
    let mut patch = fixture();
    let on = crate::signal::Cell::new(false);
    let _toggle = mount(crate::widget::toggle(on), root());
    flush(&mut patch);

    patch.clear();
    on.set(true);
    crate::signal::flush();
    flush(&mut patch);
    assert!(binds(&patch, windows_scene::Prop::OffsetX).is_empty());
    let front = Host::with(|h| h.controls.iter().next().unwrap().1.front);
    assert_eq!(front.source_fraction, 1.0);
    assert!(
        front.revision > 0,
        "the scene receives the source replacement"
    );
}

/// The knob's box and its travel survive a fractional display scale.
///
/// Layout snaps to the physical grid, so a knob sized as a fraction of the row height lands
/// on a different DIP extent at 1.5 than at 1.0 — and its travel is the difference between
/// two snapped boxes. A widget checked only at 96 DPI cannot see a knob that comes out
/// mid-track on the display the application actually runs on.
#[test]
fn a_toggles_knob_lands_at_its_travel_at_a_fractional_scale() {
    let mut patch = fixture_at(144.0);
    let on = crate::signal::Cell::new(true);
    let _toggle = mount(crate::widget::toggle(on), root());
    flush(&mut patch);

    let (rest, travel, thumb) = Host::with(|h| {
        h.controls.iter().next().map_or((0.0, 0.0, None), |(_, c)| {
            (c.front.rest, c.front.travel, c.front.thumb)
        })
    });
    let thumb = thumb.expect("a toggle mints a knob");
    let mut last = None;
    for op in patch.ops() {
        match op {
            Op::Bind {
                id,
                prop: windows_scene::Prop::Offset,
                bind: windows_scene::Bind::Set(windows_scene::Value::Vec2(v)),
            } if *id == thumb => last = Some(v.x),
            Op::Bind {
                id,
                prop: windows_scene::Prop::OffsetX,
                bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(x)),
            } if *id == thumb => last = Some(*x),
            _ => {}
        }
    }
    assert!(travel > 0.0, "the knob has room to move at 1.5");
    assert_eq!(
        last,
        Some(rest),
        "layout publishes the resting box; scene adoption places the scalar part"
    );
}

/// A washed surface paints a gradient over its own fill, resolved through the palette.
///
/// The stops name roles and strengths, so no colour crosses the authoring seam — the rule a
/// sprite's role already follows, at the one place a resource rather than a node carries it.
#[test]
fn a_wash_paints_a_ramp_over_the_surface_it_covers() {
    let (_resource_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let hue = crate::role::DataRole(1);
        let id = ramp(
            &[
                Stop {
                    at: 0.0,
                    role: hue,
                    strength: 0.06,
                },
                Stop {
                    at: 1.0,
                    role: hue,
                    strength: 0.0,
                },
            ],
            windows_scene::Spread::Horizontal,
        );
        let _mount = mount(
            crate::widget::card().washed(id, Metric::RadiusSurface),
            root(),
        );
        flush(&mut patch);

        let ramps = patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Paint {
                        paint: Paint::Ramp(_),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(ramps, 1, "the wash is one sprite painting the ramp");
        // The card keeps its own fill: a wash is a tint over a surface, not the surface.
        let solids = patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Paint {
                        paint: Paint::Solid(_),
                        ..
                    }
                )
            })
            .count();
        assert!(solids >= 2, "the card's own fill and hairline survive it");
    });
}

/// A toggle's track keeps its own box, bare and among siblings.
///
/// The widget states `RowH × 1.7` by `RowH` and nothing overrides it, but a flex row shrinks
/// a child that states no basis, and a track squeezed towards square rounds as a stadium
/// however small its radius and leaves its knob barely any travel. Both symptoms read as
/// mask bugs, so the box is asserted where it is decided.
///
/// At 144 DPI, the scale the application runs at: layout snaps to the physical grid, so a
/// box measured only at 96 is not the box that ships.
#[test]
fn a_toggles_track_keeps_its_box_among_siblings() {
    let track_box = |view: View| {
        let mut patch = fixture_at(144.0);
        let mount = mount(view, root());
        flush(&mut patch);
        let node = Host::with(|h| {
            h.controls
                .iter()
                .find(|(_, c)| c.uia == crate::widget::UiaRole::CheckBox)
                .map(|(_, c)| c.node)
        })
        .expect("a toggle mints a control");
        let size = Host::with(|h| h.model().solved(node).size);
        drop(mount);
        size
    };

    let on = crate::signal::Cell::new(true);
    let bare = track_box(crate::widget::toggle(on));
    // What the widget asks for, through the same metric it states it in. Read after the
    // first fixture, which is what installs the palette a metric resolves against.
    let track = crate::role::metric(
        Metric::TrackH,
        Scope::root(
            crate::role::tests::palette(),
            AccentId(0),
            Density::Comfortable,
        ),
    );
    // The widget's own proportion, not a copy of it: the claim is that layout gives the track
    // the box the widget asked for, and a restated number tests two copies against each other
    // instead.
    let want = Vector2 {
        x: track * crate::widget::seed::TRACK_ASPECT,
        y: track,
    };
    // The shape of a chain row's header: a spacer takes the slack, and the toggle sits
    // between a label and a fixed-width disclosure.
    let among = track_box(
        crate::layout::row((
            crate::widget::title("Parametric EQ"),
            crate::layout::spacer(),
            crate::widget::label("All"),
            crate::widget::toggle(on),
            crate::widget::button("")
                .ghost()
                .width(Len::Times(Metric::RowH, 1.0))
                .height(Len::Times(Metric::RowH, 1.0)),
        ))
        .gap(Len::Metric(Metric::SpaceSm))
        .padding(Len::Metric(Metric::SpaceSm))
        .align(crate::layout::Align::Center),
    );

    let close = |got: Vector2, what: &str| {
        assert!(
            (got.x - want.x).abs() <= 1.0 && (got.y - want.y).abs() <= 1.0,
            "the {what} toggle solved to {got:?}, not the {want:?} it asks for"
        );
    };
    close(bare, "bare");
    close(among, "sibling");
    // The two agree: a row full of neighbours does not take the track's width off it.
    assert!(
        (bare.x - among.x).abs() <= 1.0 && (bare.y - among.y).abs() <= 1.0,
        "the track is {bare:?} alone and {among:?} among siblings"
    );
}

/// A region paints its buffer and never a colour.
///
/// The sprite carries a role, because a seed states one, but that role must not reach the
/// paint: a region whose sprite resolved to a solid would cover its own buffer with the
/// surface fill and read as a flat rectangle — the same symptom as a renderer that never
/// bound, and indistinguishable from it on screen.
#[test]
fn a_region_paints_its_buffer_rather_than_its_role() {
    let mut patch = fixture();
    let live = crate::present::Live::new().expect("the epoch's wake event");
    let _mount = mount(
        crate::present::region(windows_present::Queue::Solo, &live, |_, _| {
            unreachable!("no present thread is installed in a fixture, so nothing builds")
        })
        .grow(),
        root(),
    );
    flush(&mut patch);

    let presented = patch
        .ops()
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Paint {
                    paint: Paint::Presented(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(presented, 1, "the region is one sprite painting its buffer");
}

/// A region with no box stays pending rather than allocating buffers against zero.
///
/// A node solves to no area before its first real solve, and inside anything the layout has
/// collapsed. Buffers taken from that box would be one texel across for the life of the
/// window, and nothing afterwards resizes them: the extent is only re-sent when it *moves*,
/// and a region mounted at zero has already recorded zero as sent. So the flush that gives
/// the node a box has to be the flush that mounts it.
#[test]
fn a_region_with_no_box_defers_its_buffers_until_it_has_one() {
    let mut patch = fixture();
    let live = crate::present::Live::new().expect("the epoch's wake event");
    // Collapsed before the mount, so the first solve is the one with no area to give.
    Host::with(|h| h.set_window(Vector2 { x: 0.0, y: 0.0 }));
    let _mount = mount(
        crate::present::region(windows_present::Queue::Solo, &live, |_, _| {
            unreachable!("no present thread is installed in a fixture, so nothing builds")
        })
        .grow(),
        root(),
    );
    flush(&mut patch);
    assert_eq!(
        Host::with(|h| crate::present::tests::census(h)),
        (1, 1),
        "the region is declared, and with no box it has not been mounted"
    );

    Host::with(|h| h.set_window(Vector2 { x: 800.0, y: 600.0 }));
    flush(&mut patch);
    assert_eq!(
        Host::with(|h| crate::present::tests::census(h)),
        (1, 0),
        "the flush that gives the region a box is the flush that mounts it"
    );
}

/// A contact inside a region resolves to a part, and the renderer is told directly.
///
/// The whole point of the path: the decision is written into the region's own input and its
/// epoch is bumped on this thread, so the next present carries the new pixels without the
/// application thread being involved at all.
///
/// The region is inset, and the part rects are narrow enough that the client point lands on
/// no part unless the region's origin is subtracted first. A pick that skipped that step
/// would resolve to nothing here rather than to the wrong part, which is the failure a
/// wider fixture would hide.
#[test]
fn a_contact_inside_a_region_picks_a_part_and_tells_its_renderer() {
    use windows_present::{Part, Rect, SubId};

    let mut patch = fixture();
    let live = crate::present::Live::new().expect("the epoch\'s wake event");
    let inset = crate::role::metric(Metric::SpaceLg, Host::with(|h| h.root_scope));
    let _mount = mount(
        stack(
            crate::present::region(windows_present::Queue::Solo, &live, |_, _| {
                unreachable!("no present thread is installed in a fixture")
            })
            .grow()
            .erase(),
        )
        .padding(Metric::SpaceLg)
        .grow(),
        root(),
    );
    flush(&mut patch);

    let mut hits = windows_scene::HitTable::default();
    hits.replace(patch.hit_entries());
    let id = Host::with(|h| crate::present::tests::control(h)).expect("the region is a control");
    let mut picks = picks();

    // Region-local, and deliberately narrow: at ten DIPs across, a point resolved in client
    // space misses both.
    live.parts.publish(&[
        Part {
            id: SubId(0),
            rect: Rect::new(0.0, 0.0, 10.0, 40.0),
        },
        Part {
            id: SubId(1),
            rect: Rect::new(10.0, 0.0, 20.0, 40.0),
        },
    ]);

    let mut intents = Vec::new();
    let at = windows_scene::Point {
        x: inset + 15.0,
        y: inset + 5.0,
    };
    crate::present::pick(
        &[crate::input::Report::HoverChanged {
            from: None,
            to: Some(id),
            at,
            qpc: 0,
        }],
        &hits,
        &mut picks,
        &mut intents,
    );
    assert_eq!(
        live.input.hover(),
        Some(SubId(1)),
        "the point is in the second part, once the region\'s own origin is taken off"
    );
    assert_eq!(
        live.input.cursor(),
        Some((15.0, 5.0)),
        "the cursor is published in the region\'s own DIPs"
    );
    assert!(
        intents.is_empty(),
        "a hover changes pixels and no document, so nothing is queued for the application"
    );

    // Leaving clears both. A readout drawn at the last position the pointer held while it is
    // elsewhere states a measurement nobody is taking.
    crate::present::pick(
        &[crate::input::Report::HoverChanged {
            from: Some(id),
            to: None,
            at,
            qpc: 0,
        }],
        &hits,
        &mut picks,
        &mut intents,
    );
    assert_eq!(live.input.hover(), None);
    assert_eq!(live.input.cursor(), None);
}

/// A gesture that finishes inside a region clears the active part and tells the application
/// which part it finished on.
///
/// Two separate obligations. The renderer learns immediately, through the input, so its next
/// frame stops drawing the part as held; the application learns through the ordinary intent
/// queue, on its own schedule, because the document edit is its business and not the frame
/// clock\'s.
#[test]
fn a_release_inside_a_region_clears_the_gesture_and_queues_one_intent() {
    use windows_present::{Part, Rect, SubId};

    let mut patch = fixture();
    let live = crate::present::Live::new().expect("the epoch\'s wake event");
    let _mount = mount(
        crate::present::region(windows_present::Queue::Solo, &live, |_, _| {
            unreachable!("no present thread is installed in a fixture")
        })
        .grow()
        .erase(),
        root(),
    );
    flush(&mut patch);

    let mut hits = windows_scene::HitTable::default();
    hits.replace(patch.hit_entries());
    let id = Host::with(|h| crate::present::tests::control(h)).expect("the region is a control");
    let mut picks = picks();
    live.parts.publish(&[Part {
        id: SubId(3),
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
    }]);

    let mut intents = Vec::new();
    crate::present::pick(
        &[crate::input::Report::Released {
            target: id,
            contact: 0,
            at: windows_scene::Point { x: 50.0, y: 50.0 },
        }],
        &hits,
        &mut picks,
        &mut intents,
    );
    assert_eq!(
        live.input.active(),
        None,
        "the gesture is over, so nothing is held"
    );
    assert_eq!(
        intents.len(),
        1,
        "the application is told once, after the pixels were committed to"
    );
    assert_eq!(
        intents[0].what,
        crate::widget::What::Committed(3.0),
        "the intent names the part the gesture finished on"
    );
}

/// An attached button's resting border and hover wash must end at the same square edge.
#[test]
fn edge_buttons_join_without_a_border_or_rounded_gap() {
    use crate::layout::Edge;
    use windows_scene::{Corners, Mask};
    for dpi in [96.0, 144.0, 192.0] {
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            let mut patch = fixture_at(dpi);
            let _held = mount(
                crate::widget::edge_button("", edge, crate::role::tests::CORNER)
                    .width(Metric::CardMinW)
                    .height(Metric::RowH),
                root(),
            );
            flush(&mut patch);
            let boxes: Vec<_> = patch
                .ops()
                .iter()
                .filter_map(|op| match op {
                    Op::Mask {
                        id,
                        mask: Mask::Box { radius } | Mask::Outline { radius, .. },
                        ..
                    } => Some((*id, *radius)),
                    _ => None,
                })
                .collect();
            assert_eq!(boxes.len(), 3, "border, fill and interaction wash");
            let radius =
                crate::role::metric(crate::role::tests::CORNER, Host::with(|h| h.root_scope));
            let mut expected = Corners::all(radius);
            match edge {
                Edge::Left => {
                    expected.tl = 0.0;
                    expected.bl = 0.0;
                }
                Edge::Right => {
                    expected.tr = 0.0;
                    expected.br = 0.0;
                }
                Edge::Top => {
                    expected.tl = 0.0;
                    expected.tr = 0.0;
                }
                Edge::Bottom => {
                    expected.bl = 0.0;
                    expected.br = 0.0;
                }
            }
            assert_eq!(boxes[0].1, expected);
            assert_eq!(boxes[2].1, expected);
            let (outer, fill) = Host::with(|h| {
                (
                    h.model().solved(boxes[0].0.node()).rect,
                    h.model().solved(boxes[1].0.node()).rect,
                )
            });
            let gaps = [
                fill.x0 - outer.x0,
                outer.x1 - fill.x1,
                fill.y0 - outer.y0,
                outer.y1 - fill.y1,
            ];
            let attached = match edge {
                Edge::Left => 0,
                Edge::Right => 1,
                Edge::Top => 2,
                Edge::Bottom => 3,
            };
            for (side, gap) in gaps.into_iter().enumerate() {
                if side == attached {
                    assert_eq!(gap, 0.0);
                } else {
                    let hairline =
                        crate::role::metric(Metric::HairlineW, Host::with(|h| h.root_scope));
                    assert!(
                        (gap - hairline).abs() <= 0.5 * 96.0 / dpi + 0.001,
                        "exposed border differs from its snapped hairline at {dpi} DPI"
                    );
                }
            }
            patch.clear();
            flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
    }
}

/// A responsive shadow changes visibility without reallocating or re-blurring its source.
#[test]
fn a_drawer_shadow_is_retained_across_width_classes() {
    use crate::layout::{Edge, responsive};
    use crate::role::WidthClass;
    let mut patch = fixture();
    let _held = mount(
        responsive(
            [1100.0, 1400.0],
            stack(
                crate::widget::sheet("shadow")
                    .shadowed(Edge::Left)
                    .cover()
                    .hide_when(WidthClass::Wide),
            )
            .width(Len::Pct(1.0))
            .height(Len::Pct(1.0)),
        )
        .width(Len::Pct(1.0))
        .height(Len::Pct(1.0)),
        root(),
    );
    flush(&mut patch);
    let halos: Vec<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Halo {
                id,
                halo: Some(halo),
            } => Some((*id, *halo)),
            _ => None,
        })
        .collect();
    assert_eq!(halos.len(), 1);
    let (sprite, halo) = halos[0];
    assert_eq!(halo.blur, 18.0);
    assert_eq!(halo.offset, Vector2 { x: -14.0, y: 0.0 });
    assert_eq!(halo.tint, Radiance::new(0.0, 0.0, 0.0, 0.45));
    for (width, shown) in [
        (1500.0, false),
        (1000.0, true),
        (1500.0, false),
        (1100.0, true),
    ] {
        patch.clear();
        Host::with(|h| h.model().set_window(Vector2 { x: width, y: 600.0 }));
        flush(&mut patch);
        let size = Host::with(|h| h.model().solved(sprite.node()).size);
        assert_eq!(size.x > 0.0 && size.y > 0.0, shown, "{width}: {size:?}");
        assert!(
            !patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::Halo { .. } | Op::New { .. }))
        );
        patch.clear();
        flush(&mut patch);
        assert!(patch.ops().is_empty());
    }
}

#[test]
fn slider_thumb_centres_and_fill_share_the_rail_at_every_gain() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        for dpi in [96.0, 144.0, 192.0] {
            let mut patch = fixture_at(dpi);
            let value = crate::signal::Cell::new(-4.0_f64);
            let _held = mount(
                crate::widget::slider(
                    value,
                    crate::widget::Range::new(-24.0, 24.0).step(0.1),
                    crate::widget::SliderStyle {
                        origin: Some(0.0),
                        ramp: None,
                    },
                )
                .width(Len::Pct(1.0)),
                root(),
            );
            for width in [240.0, 601.0, 940.0, 240.0] {
                Host::with(|h| h.set_window(Vector2 { x: width, y: 100.0 }));
                for db in [-24.0, -18.0, -12.0, -6.0, -4.0, 0.0, 6.0, 12.0, 18.0, 24.0] {
                    value.set(db);
                    for _ in 0..4 {
                        patch.clear();
                        crate::signal::flush();
                        flush(&mut patch);
                    }
                    Host::with(|h| {
                        let c = h
                            .controls
                            .iter()
                            .find(|(_, c)| c.front.trail.is_some())
                            .unwrap()
                            .1;
                        let front = c.front;
                        let node = c.node;
                        let (trail, origin) = front.trail.unwrap();
                        let control = h.model().solved(node);
                        let thumb = h.model().solved(front.thumb.unwrap());
                        let rail = h.model().solved(trail);
                        let fraction = ((db + 24.0) / 48.0) as f32;
                        let centre = control.rect.x0
                            + front.rest
                            + front.travel * front.source_fraction
                            + thumb.size.x * 0.5;
                        let expected = rail.rect.x0 + rail.size.x * fraction;
                        assert_eq!(origin, 0.5);
                        assert!((front.source_fraction - fraction).abs() < 1e-6);
                        assert!(
                            (centre - expected).abs() <= 96.0 / dpi,
                            "{db} dB at {width} DIP/{dpi} DPI: thumb {centre}, rail {expected}"
                        );
                        assert!((front.travel - rail.size.x).abs() <= 96.0 / dpi);
                    });
                    assert!(patch.ops().is_empty(), "settled slider emitted idle writes");
                }
            }
        }
    });
}

#[test]
fn bipolar_slider_keeps_its_value_stroke_with_the_thumb_owner() {
    let (_owner, ()) = crate::signal::Owner::scope(|| {
        let mut patch = fixture();
        let value = crate::signal::Cell::new(-12.0_f64);
        let _held = mount(
            crate::widget::slider(
                value,
                crate::widget::Range::new(-24.0, 24.0),
                crate::widget::SliderStyle {
                    origin: Some(0.0),
                    ramp: None,
                },
            )
            .width(Metric::CardMinW),
            root(),
        );
        flush(&mut patch);
        let before = filled()
            .chrome
            .into_iter()
            .find(|r| r.trail.is_some())
            .unwrap();
        let (trail, origin) = before.trail.unwrap();
        assert_eq!(origin, 0.5);
        assert!(before.thumb.is_some());
        crate::signal::flush();
        flush(&mut patch);
        patch.clear();
        value.set(12.0);
        crate::signal::flush();
        flush(&mut patch);
        let rows = filled().chrome;
        assert!(
            rows.iter()
                .any(|r| r.id == before.id && r.source_fraction == 0.75 && r.trail == before.trail)
        );
        assert!(
            !patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::New { .. } | Op::Res { .. }))
        );
        assert!(
            !patch
                .ops()
                .iter()
                .any(|op| matches!(op, Op::Bind { id, .. } if *id == trail))
        );
    });
}

#[test]
fn application_tokens_restyle_nested_scopes_without_remounting_or_idle_writes() {
    use crate::role::{ScopedToken, WidthClass};
    use crate::widget::{TextStyle, styled_text};
    use windows_text::{FamilyId, FontSpec};
    static WIDTH: ScopedToken<f32> = ScopedToken::new("test-width", |s| match s.width {
        WidthClass::Narrow => 111.0,
        WidthClass::Medium => 222.0,
        WidthClass::Wide => 333.0,
    });
    static TYPE: ScopedToken<FontSpec> = ScopedToken::new("test-type", |s| {
        FontSpec::new(
            FamilyId(0),
            match s.width {
                WidthClass::Narrow => 12.0,
                WidthClass::Medium => 18.0,
                WidthClass::Wide => 24.0,
            },
        )
    });
    for dpi in [96.0, 144.0, 192.0] {
        let mut patch = fixture_at(dpi);
        let _held = mount(
            crate::layout::responsive(
                [600.0, 1000.0],
                stack((
                    plate().width(Metric::Custom(&WIDTH)).height(Metric::RowH),
                    styled_text(
                        "Responsive typography",
                        TextStyle::new(TypeRole::Custom(&TYPE)),
                    ),
                    crate::layout::responsive(
                        [600.0, 1000.0],
                        plate().width(Metric::Custom(&WIDTH)).height(Metric::RowH),
                    )
                    .width(Len::Pct(0.4)),
                )),
            )
            .width(Len::Pct(1.0)),
            root(),
        );
        flush(&mut patch);
        let ids = Host::with(|h| h.mounts.iter().map(|(_, m)| m.node).collect::<Vec<_>>());
        let mut narrow_text_width = None;
        for (width, expected, text_scale) in [
            (500.0, 111.0, 1.0),
            (800.0, 222.0, 1.5),
            (1200.0, 333.0, 2.0),
            (500.0, 111.0, 1.0),
        ] {
            patch.clear();
            Host::with(|h| h.model().set_window(Vector2 { x: width, y: 700.0 }));
            flush(&mut patch);
            Host::with(|h| {
                let now = h.mounts.iter().map(|(_, m)| m.node).collect::<Vec<_>>();
                assert_eq!(ids, now, "resize retains every node");
                let measured = h.model().solved(ids[2]);
                assert!(
                    (measured.size.x - expected).abs() < 1.0,
                    "{dpi}/{width}: {:?}",
                    measured.size
                );
                let nested = h.model().solved(ids[5]);
                assert!(
                    (nested.size.x - 111.0).abs() < 1.0,
                    "nested scope stays narrow"
                );
                let text_width = h.model().solved(ids[3]).size.x;
                let narrow = *narrow_text_width.get_or_insert(text_width);
                assert!(
                    (text_width - narrow * text_scale).abs() < 2.0,
                    "typography follows the solved class: {text_width} / {narrow}"
                );
            });
            assert!(
                !patch
                    .ops()
                    .iter()
                    .any(|op| matches!(op, Op::New { .. } | Op::Paint { .. })),
                "a resize neither remounts nor rebinds paint"
            );
            patch.clear();
            flush(&mut patch);
            assert!(
                patch.ops().is_empty(),
                "unchanged tokens emit no idle writes"
            );
        }
    }
}

#[test]
fn application_text_ink_survives_wrapping_and_ellipsis() {
    use crate::widget::{TextStyle, shown, styled_text};
    let ink = Role::Data(crate::role::DataRole(123));
    for flow in [Flow::Wrap, Flow::Ellipsis] {
        let mut patch = fixture();
        let value = crate::signal::Cell::new("A chromatic application label");
        let _held = mount(
            styled_text(
                shown(move || value.get()),
                TextStyle {
                    ink: Some(ink),
                    flow,
                    ..TextStyle::new(TypeRole::Body)
                },
            )
            .width(Len::Pct(1.0)),
            root(),
        );
        let expected = crate::role::resolve(ink, root_scope());
        let mut paints = 0;
        for width in [800.0, 80.0] {
            Host::with(|h| h.model().set_window(Vector2 { x: width, y: 600.0 }));
            flush(&mut patch);
            for op in patch.ops() {
                if let Op::Paint {
                    paint: Paint::Solid(light),
                    ..
                } = op
                {
                    assert_eq!(
                        *light, expected,
                        "{flow:?}: a line lost its application colour"
                    );
                    paints += 1;
                }
            }
            patch.clear();
            value.set("A much longer chromatic application label for reflow");
            crate::signal::flush();
        }
        assert!(paints > 0);
    }
}
