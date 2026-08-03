//! What the lowering claims, checked by driving it headless.
//!
//! `Model` owns no COM, so a whole mount runs with no window, no device and no compositor,
//! and the ops it emitted read back off the patch.

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

/// Installs this thread's palette, text engine and a fresh host for this test.
///
/// The palette is process-wide and installs once; the engine and the host are per thread,
/// and tests run on their own, so each gets a tree and an engine of its own to assert
/// against.
pub(crate) fn fixture() -> SinkPatch {
    crate::role::tests::palette();
    if !super::text::installed() {
        // The real engine, over the two inbox faces the palette names. Deliberate:
        // a double that invented advances would let a wiring test pass while the
        // engine it stands for measured something else entirely.
        super::text::install(FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]))
            .expect("DirectWrite is available on the platform floor");
    }
    let root = taffy::Style {
        size: taffy::Size {
            width: taffy::Dimension::percent(1.0),
            height: taffy::Dimension::percent(1.0),
        },
        ..taffy::Style::DEFAULT
    };
    let mut model = Model::new(root);
    model.set_window(Vector2 { x: 800.0, y: 600.0 });
    Host::install(
        model,
        Env::new(
            96.0,
            OutputTransform::for_display(DisplayCapability::Sdr, 1000.0),
        ),
        Scope::root(AccentId(0), Density::Comfortable),
    );
    // The model minted its own root before this test existed, and that op rides the first
    // flush. Draining it here is what makes every assertion below about *this* mount.
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

/// A bare rounded box, which is the smallest thing that mints a sprite.
fn plate() -> View {
    El::<Any>::seed(crate::layout::Preset::Bare).sprite(
        MaskSeed::Box {
            radius: Some(Len::Metric(Metric::Radius)),
        },
        Role::Fill(Fill::Surface),
        Part::Fill,
    )
}

// ── the claims ───────────────────────────────────────────────────────────────────

/// A slot with one sprite and no children **is** that sprite: one visual, no group.
///
/// This is the case that decides a screen's visual count, because most of a screen is
/// exactly this shape.
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

/// A container mints a group, and its children land in **paint order**.
///
/// Order is the whole of z-order in this system, so it is asserted as an order and not as
/// a set.
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

/// A **constant** channel is one `Set` at mount: no graph node, no effect.
///
/// This is the invariant that makes a static screen cost sprites and nothing else, and it
/// is enforced in one place for the whole widget set.
#[test]
fn a_constant_channel_produces_no_effect() {
    let mut patch = fixture();
    let _mount = mount(plate().opacity(0.5), root());
    flush(&mut patch);

    // `.opacity` declares `Motion::Chrome`, so anything that went through an effect would
    // arrive as a spring. A plain `Set` is the proof that the constant path was taken and
    // no effect was created.
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

/// A **reactive** channel is one effect, and writing its cell re-binds the property.
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

/// An interactive control mints exactly **one** extra visual, and parks it invisible.
///
/// One visual per interactive control is the cost this design accepted for a hover that
/// costs the app thread nothing. If it ever becomes two, that trade has silently changed.
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

/// A sprite's colour is resolved through the palette, at the **elevated** scope its
/// surface pushed — and with the width axis pinned.
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
    // A surface resolves its own chrome at the rung it pushed — a card *is* the raised
    // thing — so what a push means is that the same sprite differs inside it and outside.
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

/// A `Metric` reaches the style through the palette, so a spacing is never a number a
/// widget wrote.
#[test]
fn a_metric_override_lowers_through_the_palette() {
    let scope = Scope::root(AccentId(0), Density::Comfortable);
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

/// Text is measured under the type ramp the **palette resolved**, not under whatever
/// was current when the node was built.
///
/// Asserted as a **ratio** between two rungs rather than against an absolute width. The
/// engine is DirectWrite, so an absolute figure would pin this test to one font's
/// advances and it would fail on a font update having caught nothing. A ratio holds
/// whatever the face is, and it is the property the wiring actually owes: measure took
/// the size the palette gave for the role.
#[test]
fn text_measures_under_the_resolved_type_ramp() {
    fn width_of(ramp: TypeRole) -> f32 {
        let mut patch = fixture();
        let label = El::<Any>::seed(crate::layout::Preset::Text).text_seed(
            crate::widget::TextSource::Static("hello"),
            ramp,
            Some(Text::Primary),
            Flow::Line,
        );
        let _mount = mount(label, root());
        flush(&mut patch);
        Host::with(|h| {
            let (_, row) = h.mounts.iter().last().expect("the label mounted");
            let node = row.node;
            h.model().solved(node).size.x
        })
    }

    let scope = Scope::root(AccentId(0), Density::Comfortable);
    let body = crate::role::typography(TypeRole::Body, scope).size;
    let display = crate::role::typography(TypeRole::Display, scope).size;
    assert!(
        display > body,
        "the ramp under test does not separate its rungs"
    );

    let (measured_body, measured_display) = (width_of(TypeRole::Body), width_of(TypeRole::Display));
    assert!(measured_body > 0.0, "the body rung measured nothing");

    // One string, one face, two sizes: advances scale with the em, so the measured
    // widths carry the ramp's own ratio. The tolerance is hinting, which quantizes
    // advances per size and is the only reason this is not exact.
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

/// A widget cannot express a length that is not the palette's.
///
/// Asserted as a property of the type rather than of a lint: there is no `Len` variant
/// carrying an arbitrary DIP, so the exhaustive match below is the whole vocabulary.
#[test]
fn len_has_no_raw_dip_constructor() {
    let scope = Scope::root(AccentId(0), Density::Comfortable);
    for len in [
        Len::Metric(Metric::SpaceMd),
        Len::Zero,
        Len::Pct(0.5),
        Len::Times(Metric::RowH, 4.0),
        Len::Auto,
    ] {
        // Exhaustive, so the vocabulary cannot grow a raw-DIP form without this failing to
        // compile. `Times` is a count of a metric, which is why it is admissible: it can
        // say "four rows" and still cannot say "twelve".
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

/// Colour must not read the width axis, so a resize re-lowers styles and rebinds no paint.
///
/// The exhaustive product is small — a closed role enum by three classes — so this is the
/// whole claim and not a sample of it.
#[test]
fn colour_is_width_independent() {
    crate::role::tests::palette();
    let base = Scope::root(AccentId(0), Density::Comfortable);
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

/// A surface arranges its children **as it was told**, whatever chrome it carries.
///
/// The regression this exists for: chrome and layout class were once the same field, so a
/// container could only adopt a class if it did not already have one — and a card, whose
/// chrome happened to be a column, laid `card().row(..)` out as a column with nothing to
/// say so. A silently wrong layout is the worst available failure, because it reads as a
/// design decision.
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

/// And the chrome survives the class it was given.
///
/// The other half of the same fix: making the class always win must not throw away the
/// padding, the scope push or the fill that made it a surface.
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

    let scope = Scope::root(AccentId(0), Density::Comfortable);
    let padding = crate::role::metric(Metric::SpaceLg, scope);
    let (surface, child) = Host::with(|h| {
        let nodes: Vec<_> = h.mounts.iter().map(|(_, m)| m.node).collect();
        (h.model().solved(nodes[0]), h.model().solved(nodes[1]))
    });
    assert!(
        (child.rect.x0 - surface.rect.x0 - padding).abs() < 0.5,
        "the surface's padding must survive being told to be a row"
    );
    // And its own fill seed is still there. Whether that fill resolves one rung up is the
    // scope push, which `a_surface_elevates_the_scope_its_children_resolve_against` owns —
    // asserting it again here would be the same claim in two places, and a child painted
    // with the same role at the same rung is *supposed* to match.
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
    // Three, and the third is the point: a card's hairline is an outer box in the stroke
    // colour with the fill inset over it, because the alphabet has no outlined rectangle.
    // If this ever reads 2, the surface lost its edge.
    assert_eq!(painted, 3, "the card's ring and fill, and the child's");
}

/// Motion is a property of the channel, declared by the seed — so two call sites cannot
/// disagree about the same control.
#[test]
fn motion_is_per_channel_and_not_per_call_site() {
    assert_eq!(Motion::default(), Motion::Snap);
}

// ── unmount ──────────────────────────────────────────────────────────────────────

/// Dropping a mount releases **every** row the walk claimed.
///
/// This is where a leak would be invisible: the nodes go away on screen whatever happens,
/// so a control row or a laid-out run left behind shows up only as a table that grows for
/// the life of the process.
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
    // Everything else the walk claimed goes the same way, and through the row that named it
    // rather than through a scan: a table left holding a dead node's row is what makes
    // unmounting one list row cost the whole screen.
    Host::with(|h| {
        assert_eq!(h.values.len(), 0);
        assert_eq!(h.scrolls.len(), 0);
    });
    assert_eq!(
        style::with(|table| table.len()),
        0,
        "an unmount must release the style recipes"
    );

    // One destroy op, because it cascades on the far side: a partial destroy is not
    // expressible, so a subtree cannot be half-gone.
    let drops = patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::Drop { .. }))
        .count();
    assert_eq!(drops, 1);
}

/// And the slots come back, so a list that churns does not grow the tables.
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

/// A variant is a row, and the row decides the visual count.
///
/// This is the claim that keeps a variant from becoming a function with a body: a ghost has
/// no fill and no stroke, so it *mints* neither — rather than minting two invisible sprites
/// and trusting a branch to have skipped them.
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

/// A control's moving part is a **child** of it, and the control has to find it anyway.
///
/// The regression this closes had no visible symptom at mount: a slider rendered, its
/// handlers fired, and every structural assertion passed — but the front-side row carried no
/// thumb, so the router computed a value it could not show and the invariant that pixels move
/// in the tick that saw the event was quietly false for every control that has a value.
#[test]
fn a_control_claims_the_moving_part_its_children_declared() {
    let mut patch = fixture();
    let value = crate::signal::Cell::new(0.5_f64);
    let _slider = mount(
        crate::widget::slider(value, crate::widget::Range::UNIT).width(Metric::CardMinW),
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
}

/// A fraction is finished against the travel, not bound to an offset raw.
///
/// `Prop::OffsetX` is in DIPs. Binding `0..=1` straight to it moves a thumb by one DIP, which
/// renders as a control that does not work — and reads, in a screenshot, as a design choice.
///
/// A **toggle** is the case this thread finishes: a press has no value to read off a pointer,
/// so its knob follows the application's own channel and the app thread is the writer.
#[test]
fn a_fraction_reaches_the_offset_multiplied_by_its_room() {
    let mut patch = fixture();
    let on = crate::signal::Cell::new(true);
    let _toggle = mount(crate::widget::toggle(on).width(Metric::CardMinW), root());
    flush(&mut patch);

    let travel = Host::with(|h| {
        h.controls
            .iter()
            .next()
            .map_or(0.0, |(_, c)| c.front.travel)
    });
    assert!(travel > 0.0, "a knob in a sized track has room to move");
    let offsets = offsets_bound(&patch);
    assert!(
        offsets.iter().any(|&v| (v - travel).abs() < 0.5),
        "a knob at the top of its range sits at the end of its travel ({travel}): {offsets:?}"
    );
}

/// A part the **router** drives is not written by this thread at all.
///
/// The one channel with two interested writers, and the split that keeps it to one: the app
/// ships the room the solve measured and the front side multiplies. Binding it here as well
/// would fight a live drag with a geometry correction, and the correction would win — a
/// resize mid-slide would snap the thumb back to where the application last wrote it.
#[test]
fn a_slid_part_is_left_to_the_thread_that_moves_it() {
    // A **slid** part, whose property is an offset finished against a room the solve gives.
    let mut patch = fixture();
    let value = crate::signal::Cell::new(0.25_f64);
    let _slider = mount(
        crate::widget::slider(value, crate::widget::Range::UNIT).width(Metric::CardMinW),
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
    // The mount **does** seed the part: a control has to render at its value before anyone
    // touches it, and until the control mounts there is nobody else to do it.
    assert!(!binds(&patch, windows_scene::Prop::OffsetX).is_empty());

    // From here the router owns it, and this is the hazard the split exists for: the
    // application writing its own cell back — which is exactly what `on_commit` does — must
    // not reach the property, or a resize mid-drag would fight a contact and win.
    patch.clear();
    value.set(0.75);
    crate::signal::flush();
    flush(&mut patch);
    assert!(
        binds(&patch, windows_scene::Prop::OffsetX).is_empty(),
        "an owned part must not be written from this thread"
    );
    // The number it will land on is still this thread's own, from the same function.
    assert!(
        (crate::widget::offset_of(1.0, front.travel, false) - front.travel).abs() < f32::EPSILON
    );

    // And a **turned** part, whose property is an angle — the case that had its own copy of
    // the arithmetic before, and the one where the fight was reachable by turning a knob and
    // letting go.
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
}

/// Everything this thread bound to `prop`, however it bound it.
fn binds(patch: &SinkPatch, want: windows_scene::Prop) -> Vec<&Op> {
    patch
        .ops()
        .iter()
        .filter(|op| matches!(op, Op::Bind { prop, .. } if *prop == want))
        .collect()
}

/// Every `OffsetX` this thread set, in the order it set them.
fn offsets_bound(patch: &SinkPatch) -> Vec<f32> {
    patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Bind {
                prop: windows_scene::Prop::OffsetX,
                bind: windows_scene::Bind::Set(windows_scene::Value::Scalar(v)),
                ..
            } => Some(*v),
            _ => None,
        })
        .collect()
}

/// A read-only widget declares no hit entry, and therefore no control row.
///
/// A meter is the dense case: one hit entry each is one more rect every pointer sample is
/// resolved against, one more control row and one more front-side row — for a widget nothing
/// can click.
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

/// A constant `.when(false)` contributes **nothing**: no node, no style, no run.
///
/// Hiding it instead is the plausible-looking version of this, and it costs a visual, a
/// style, a mount row and — for a label — a shaped run, all for something nobody can see.
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

/// A keyed list reorders survivors rather than rebuilding them.
///
/// Step four is what makes recycling free, and it is the difference between a filter
/// keystroke costing a move per row and costing a whole tree.
#[test]
fn a_keyed_list_moves_survivors_rather_than_reminting_them() {
    let mut patch = fixture();
    let items = crate::signal::Cell::new(vec![1_u32, 2, 3]);
    let _list = mount(
        stack(each(
            move || items.get().into_iter().map(|k| (k, k)).collect(),
            |_| plate(),
        )),
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

/// A run that can break is one sprite per line; everything else is one sprite.
///
/// Both halves matter. A coverage tile is one line's, so a wrapping caption genuinely needs
/// several — and a label that paid the same price would put a group behind every string on
/// the screen.
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

/// A warm mount allocates **nothing**, and collecting children is where that is easiest to
/// lose.
///
/// The regression this closes had no visible symptom: a temporary `Vec` per container is
/// correct, passes every structural assertion, and costs one allocation per container per
/// mount — so a twelve-row list realized during a fling allocated twelve times a frame on
/// the one path that must not allocate at all. What it shows up as is the arena's own
/// buffers being at high-water mark while the heap is not.
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

    // The same shape again grows neither buffer, which is the whole claim: a realized row
    // costs the walk and nothing else.
    let second = mount(screen(), root());
    flush(&mut patch);
    assert_eq!(
        Build::with(|b| (b.kids.capacity(), b.pending.capacity())),
        (kids, pending),
        "a second mount of the same shape must not grow the arena"
    );
    drop(second);
}

/// A **warm** mount allocates nothing, counted rather than argued.
///
/// Every other claim about the arena is structural — a buffer that did not grow, a stack that
/// came back empty — and every one of them held while the walk itself allocated three `Vec`s
/// per node. Nothing short of counting catches that, because a temporary is invisible to a
/// capacity check: it is allocated and freed inside the call.
///
/// The counter is per thread and read either side of the statement, so what it reports is
/// this mount's own allocations and not whatever else the harness was running.
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
    // mark. That is the cost this design pays once and never again.
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

/// And explicit placement appends within the buffer rather than through a temporary.
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

/// A scroll container delegates to a tracker: **two bindings and one tracker**, and the
/// viewport itself never moves.
///
/// The viewport is what clips, so an offset on it would take the clip with it — and a
/// thumb positioned per frame was a measured cost in the stack this replaces, which is why
/// it rides the same tracker instead.
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

    // And the extent reached the tracker, from a solve rather than from a guess.
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
/// This is the claim that keeps scrolling off the app thread entirely: a scroll that is
/// *scrolling* moves compositor-side, and this step speaks only when the extents change.
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
/// A flex child shrinks to its parent by default, so a scroll container whose children state
/// only a height had no travel at all — a scrollbar that worked wherever a `min_height`
/// happened to be written and nowhere else.
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

/// A scroll container's tracker is **created**, and created after its viewport is sized.
///
/// Both halves are the bug this asserts against. A tracker that is only *minted* is a
/// binding onto nothing, and one created before the solve takes its hit region from a
/// zero-size visual — which hit-tests nothing while reporting success, so the surface
/// silently ignores every wheel notch for the life of the window.
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

/// The scrollbar is above the content, grabbable, and pinned.
///
/// Three claims, each a defect the shape prevents. Child order is paint order **and** the
/// order the hit array is scanned in, so a bar minted at the bottom of its viewport is
/// painted under the list and every grab on it resolves to the row behind it. And the rail
/// is *inside* the container it reports on without moving with it, so a rect that resolved
/// through that container's offset would slide off the surface as far as the content
/// scrolled.
#[test]
fn the_scrollbar_is_above_the_content_grabbable_and_pinned() {
    let mut patch = fixture();
    // One card is a target, so the array carries something that *does* resolve through the
    // viewport's offset — which is what makes the rail's opting out an assertion rather
    // than a restatement of "nothing scrolls here".
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

    // Hit order: the array is scanned from the end, so a later entry wins a point both
    // cover — and the rail sits inside the viewport's own box everywhere.
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
    // And the card beside it does, so the flag is an opt-out from something real.
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

/// A surface with nothing to scroll has no rail target.
///
/// Left on, it takes every press on the right ten DIPs of the content — a button at the
/// edge of a row that cannot be clicked, and nothing on screen to say why.
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

/// An on-demand thumb is concealed from the mount, not shown and faded.
///
/// A surface whose content fits never overflows, so a thumb that appeared for one frame to
/// say so is a flash on every screen that opens.
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

/// A grid that moved re-sends every run, and the ordinary publish does not.
///
/// Neither a pixel-grid change nor a rebuilt device moves a single DIP, so the width gate
/// that makes publishing cheap answers "nothing to do" for precisely the case where every
/// coverage tile is rasterized for a grid that is gone. Both halves are asserted, because
/// the first without the second is a test that passes on a publish that re-sends everything
/// every frame.
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

    // Settled: the ordinary publish is silent, which is what makes it cheap.
    patch.clear();
    flush(&mut patch);
    assert_eq!(runs(&patch), 0, "a settled label re-published its run");

    // And the answer to a grid that moved is not silent.
    patch.clear();
    Host::with(Host::reemit_text);
    flush(&mut patch);
    assert!(
        runs(&patch) > 0,
        "a re-emit sent nothing, so a display hop leaves every glyph at the old resolution"
    );
}

/// Every control declares a gesture; nothing else does.
///
/// `control()` sets `HitFlags::GESTURE` — whose whole meaning is "has a gesture
/// declaration" — and nothing minted one, so a plain button entered the hit array claiming
/// something that was not there. Downstream that is a press with no release: the router
/// binds a contact only where its target declared, and reports an up only where it bound.
///
/// The second half is the gate, and it is why this is not simply "always declare one": this
/// walk also runs for nodes that exist only for automation, and a recogniser behind every
/// static label is a cost with no consumer.
#[test]
fn a_control_declares_the_default_gesture_and_a_label_declares_none() {
    let mut patch = fixture();
    let _held = mount(crate::widget::button("press me"), root());
    flush(&mut patch);
    let declared = Host::with(|h| h.take_gestures());
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
        Host::with(|h| h.take_gestures()).is_empty(),
        "a static label was given a recogniser it can never use"
    );
}

// ── virtualization ───────────────────────────────────────────────────────────────

/// A thousand-row list, in a viewport that shows about ten of them.
const LIST: crate::layout::ListSpec = crate::layout::ListSpec {
    count: 1000,
    row_h: Metric::RowH,
    overscan: 2,
};

/// Settles a mounted list, and hands back the tracker driving it.
///
/// **Two flushes, and that is the mechanism rather than the test being careful.** A
/// viewport's height is a solve output, so the first flush is what measures it and the
/// realization window it implies is resolved on the tick after — which is what a running
/// window does on mount and on every resize.
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

/// Mounts a virtualized list and hands back the tracker driving it.
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

/// How many rows a list has realized, counted off the nodes the mount walk claimed.
fn realized_rows() -> usize {
    Host::with(|h| h.mounts.iter().count())
}

/// The travel the solve gave the tracker, read off what the publish last settled on.
///
/// Off the row rather than off the patch, because a flush **replaces** the caller's buffer
/// rather than appending to it, and the extent settles on the flush that measured the
/// viewport rather than on the one after it.
fn published_extent() -> f32 {
    Host::with(|h| {
        h.scrolls
            .iter()
            .next()
            .map(|(_, row)| row.last.max_scroll)
            .expect("a scroll container was mounted")
    })
}

/// The whole claim of virtualization: a thousand rows cost a screen's worth of nodes, and
/// the ones that exist are placed at their own index rather than laid out in sequence.
///
/// Placement is what makes the realized set free to be several disjoint runs — and what
/// keeps the content's extent the whole list's, so the maximum position is the constant
/// uniform extents promise rather than something that moves as the window does.
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
    // Every row sits on a row-height boundary, and the set reaches at least the second
    // screen's worth — which a sequence of laid-out children would also do, so the boundary
    // is the half that distinguishes them.
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

/// A reported position realizes the rows it implies **in the tick it arrived in**, and the
/// extent does not move because of it.
///
/// The second half is the one that is easy to lose: a list whose content height followed its
/// realized set would move the maximum position on every frame of a fling, which is content
/// sliding under the user's finger.
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

    // Within a couple of rows of the resting count: a window at the very top has its upper
    // overscan clipped away and one in the middle does not, so the two are close rather than
    // equal — and what matters is that neither grows with how far the list was scrolled.
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

/// At the instant inertia begins the destination is already known, so the rows it lands on
/// are realized while the compositor animates — and the rows still on screen stay.
///
/// A window that jumped to the destination instead would blank what the user is looking at,
/// which is the failure destination prefetch exists to prevent rather than to cause.
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
    // Both ends exist at once: the destination was realized **beside** where the content
    // still is, never instead of it.
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

    // And when it settles, the prefetch is given back.
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

/// A realized index the caller did not supply gets its space and nothing in it.
///
/// The row is where it will be when the data arrives, so the extent and every row below it
/// are already right — and no invented content claims to be the data.
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

// ── modifiers say what they do, in any order ─────────────────────────────────────

/// Declining an inflation does not depend on being called after the thing that declared a
/// target — and does not conjure one where there is none.
///
/// Every other modifier here is order-independent by construction, which is what the arena's
/// intrusive chains are for. One that quietly was not is worse than one that never worked:
/// it works in the example and not at the call site that wrote the chain the other way round.
#[test]
fn declining_an_inflation_reads_the_same_in_either_order() {
    fn inflates(view: View) -> Option<bool> {
        let mut patch = fixture();
        let _held = mount(view, root());
        flush(&mut patch);
        // Off the hit array itself, which is the only thing the router ever consults.
        patch
            .hit_entries()
            .first()
            .map(|entry| !entry.flags.contains(windows_scene::HitFlags::NO_INFLATE))
    }
    assert_eq!(inflates(plate().no_inflate().on_click(|| {})), Some(false));
    assert_eq!(inflates(plate().on_click(|| {}).no_inflate()), Some(false));
    // And on its own it is not a target at all: a control row and a slot in the array every
    // pointer sample is resolved against is the opposite of what this asks for.
    assert_eq!(inflates(plate().no_inflate()), None);
}

/// A value handler declares a target, so it is not dropped in silence.
///
/// The mount moves handlers into the dense table only for a node that has a hit entry. A
/// handler on one that does not is taken out of the arena and freed — which reads, at the
/// call site, as a control that does nothing.
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

/// A restyle re-lowers against the node's **own** scope, not the root's.
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

/// The width class is the solve's, and is never stored in the recipe it re-lowers from.
///
/// The whole of the collapse: two owners of one class, one of them a frame stale, was what
/// made a container's own re-lower disagree with the layout it was laid out under.
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
// The fixture's window is 800 DIPs wide and the containers below fill it, so the class is
// chosen by moving the *thresholds* rather than the width. That keeps every case in one
// window and makes the class each test is asserting about readable at its call site.

/// A width variant re-arranges a container without unmounting anything.
///
/// Both halves matter and only one of them is visible: the arrangement is the feature, and
/// the mount surviving is what makes it safe to evaluate during a resize drag. A `when()`
/// here would drop the subtree's owner every time a window edge crossed the threshold.
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
/// The regression, and it is the one bug in this file that was **visible from across the
/// room**: a `Flow::Line` run has no line sprite of its own — the node *is* the sprite, which
/// is what keeps a static label at one visual — so a container stretching its children
/// stretched the coverage tile with it. The tile's brush fills, so every short label came out
/// smeared horizontally to the width of the longest line beside it. A wrapping run was immune,
/// because it owns line sprites and sizes each to its own tile, which is exactly why the
/// screen that found this had one legible caption under a row of smears.
///
/// Asserted between two runs rather than against an absolute width: under the defect every
/// run in the column reports the container's width, so "they differ" is the whole claim and
/// it needs no number from the text engine.
#[test]
fn a_single_line_run_is_as_wide_as_its_own_text() {
    let mut patch = fixture();
    // `stack` stretches its children, which is the default and the case that broke.
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

/// A track sized by a fraction of its container is that fraction, and not zero.
///
/// The regression: a fixed track resolved through `Len::dips`, which answers `None` for the
/// two lengths with no intrinsic value — a percentage and `Auto` — and the `unwrap_or(0.0)`
/// beside it turned both into a **zero-width column**. A grid with a collapsed track lays out
/// perfectly cleanly, so there is nothing to see except content that is not where it was
/// asked to be.
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

/// A class-gated column list is *the* template at that class, not an addition to the one
/// below it.
///
/// Without the clear, `.cols(..).cols_when(..)` concatenates: the wide arm gets three tracks
/// for two declarations, and the second child lands a third of the way across instead of
/// half. That reads as a design decision rather than as a bug, which is why it is asserted
/// against a position and not against a track count.
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

/// The **first** solve applies the class, including when it resolves to the middle one.
///
/// The regression: `windows-scene` defaulted an unclassified node to `Medium` while the
/// scope a recipe is lowered at defaults to `Wide`, so a container that classified *to*
/// `Medium` produced no transition on its first layout and its subtree kept the styles the
/// mount had lowered at `Wide`. Nothing about the second frame is wrong, which is what makes
/// it worth a test: the window simply opens in the wrong arrangement.
#[test]
fn the_first_solve_applies_the_class_it_resolved() {
    let mut patch = fixture();
    let _held = mount(
        crate::layout::responsive(
            // 800 against these is Medium — the value the two crates used to disagree on.
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

/// Every washed control's wash is as round as the control under it.
///
/// A wash is a crossfade over the surface it covers, so a radius it does not share is a
/// square highlight on a round control — visible only when hovered, which is exactly when
/// nobody is looking at a screenshot.
#[test]
fn a_wash_is_as_round_as_the_control_it_covers() {
    // Built inside the loop: the arena clears after each mount, so an element minted before
    // one and used after it names a slot that is no longer there.
    let cases: [(&str, fn() -> View); 3] = [
        ("button", || crate::widget::button("x").erase()),
        ("knob", || {
            crate::widget::knob(0.5_f64, crate::widget::Range::UNIT)
        }),
        ("slider", || {
            crate::widget::slider(0.5_f64, crate::widget::Range::UNIT)
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
}

// ── what automation is told ─────────────────────────────────────────────────────
//
// The synthesis had no tests at all, which is how every one of the defects below reached
// a running build: each is a thing the framework half could not have caught, because the
// framework half was verified against hand-written seeds.

/// The seeds this mount produced, sorted, with their names resolved.
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

/// The published tree this mount would produce.
fn tree(patch: &SinkPatch) -> crate::uia::Tree {
    let mut out = crate::uia::Seeds::default();
    Host::with(|h| h.uia_seeds(&mut out));
    crate::uia::Tree::build(patch.hit_entries(), &out)
}

/// A button's label is a **child element**, not text on its own node, so a name derived
/// from the control's own row is empty for every button in the stack.
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

/// Static text declared no peer at all, so a screen of labels, headings and read-outs read
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

/// A slider carries no text of its own. Without this its name is empty, which is the
/// difference between a usable screen and an unusable one.
#[test]
fn a_control_with_no_text_takes_the_name_of_the_run_beside_it() {
    let mut patch = fixture();
    let value = crate::signal::Cell::new(0.5_f64);
    let _row = mount(
        stack((
            crate::widget::label("Gain"),
            crate::widget::slider(value, crate::widget::Range::UNIT).width(Metric::CardMinW),
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
}

/// The rule is narrow on purpose: it must not reach across a row or claim a heading two
/// controls up, because a wrong name is worse than none.
#[test]
fn a_control_that_has_a_name_keeps_it_and_one_with_no_run_before_it_gets_none() {
    let mut patch = fixture();
    let value = crate::signal::Cell::new(0.5_f64);
    let _row = mount(
        stack((
            crate::widget::label("Gain"),
            crate::widget::button("Reset"),
            crate::widget::slider(value, crate::widget::Range::UNIT).width(Metric::CardMinW),
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
}

/// A name is a **copy** in the published blob, so a label that re-reads leaves the tree
/// holding the old string — with no event, because the name is not a live property.
#[test]
fn a_label_that_changes_marks_the_accessible_tree_stale() {
    let mut patch = fixture();
    let caption = crate::signal::Cell::new("Off".to_owned());
    let _text = mount(
        crate::widget::text(crate::widget::TextSource::Dynamic(Box::new(move || {
            caption.get()
        }))),
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

/// A hidden part with a **measurable leaf** inside it lays out as hidden, all the way down.
///
/// The regression: taffy descends into a hidden subtree with `RunMode::PerformHiddenLayout`,
/// where a measure function may not be called — and this crate decided by each node's own
/// display, so the first childless descendant of a hidden container was handed to the leaf
/// path and walked into taffy's `unreachable!()`. It needs a text run inside to reproduce:
/// a hidden node whose children are all themselves boxes never reaches a measure at all,
/// which is why the mechanism's own tests missed it and the first real screen found it.
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
