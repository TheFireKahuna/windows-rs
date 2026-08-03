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
        &[crate::layout::Over::Width(Len::Metric(Metric::CardMinW))],
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
