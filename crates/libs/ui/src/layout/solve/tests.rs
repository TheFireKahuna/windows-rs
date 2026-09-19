//! Headless claims about the three walks, over a host with no window and no device.

use super::{shift, snap, solve_root, take_asks, take_roots, take_visits};
use crate::build::Host;
use crate::build::tree::{self, Geom};
use crate::layout::{Align, Layout, Len, Position, Preset, Rect, Track, WidthClass};
use crate::role::{AccentId, Density, Metric, Scope, ScopedToken};
use windows_color::{DisplayCapability, OutputTransform};
use windows_numerics::Vector2;
use windows_scene::{Env, GroupId, NodeId, SinkPatch};
use windows_text::FontLadder;

/// A metric that differs by width class, so a class flip moves a leaf whose own inputs did
/// not change.
static BY_CLASS: ScopedToken<f32> = ScopedToken::new("by-class", |scope| match scope.width {
    WidthClass::Narrow => 60.0,
    WidthClass::Medium => 40.0,
    WidthClass::Wide => 20.0,
});

/// A column ladder: one column narrow, two medium, three wide.
static LADDER: ScopedToken<&'static [Track]> =
    ScopedToken::new("ladder", |scope| &COLUMNS[..=scope.width as usize]);

/// Three equal columns; the ladder takes a prefix of it, so each class names `'static` data.
const COLUMNS: [Track; 3] = [Track::fr(1.0), Track::fr(1.0), Track::fr(1.0)];

const WINDOW: Vector2 = Vector2::new(600.0, 400.0);

/// Installs a host at `dpi` with a window of [`WINDOW`], and answers its root.
fn fixture(dpi: f32) -> NodeId {
    Host::install(
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
    Host::with(|h| {
        h.text
            .install(FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"]))
            .expect("a text engine");
    });
    window(WINDOW.x, WINDOW.y);
    root()
}

/// Gives the window root a client extent.
fn window(w: f32, h: f32) {
    Host::with(|host| {
        host.set_window(Vector2::new(w, h));
        host.tree.author(root(), |l| {
            l.width = Len::dip(w);
            l.height = Len::dip(h);
        });
    });
}

/// The window root, which every host mints first.
fn root() -> NodeId {
    NodeId::FIRST
}

/// Mints a child of `parent` with `write` applied to `preset`'s row.
fn node(parent: NodeId, preset: Preset, write: impl FnOnce(&mut Layout)) -> NodeId {
    Host::with(|h| {
        // Appended: `after: None` links at the front, which would reverse the siblings.
        let last = h.tree.children(parent).last();
        let id = h.group(GroupId(parent), last).0;
        h.tree.author(id, |l| {
            *l = Layout::of(preset);
            write(l);
        });
        id
    })
}

fn sized(parent: NodeId, w: f32, h: f32) -> NodeId {
    node(parent, Preset::Layer, |l| {
        l.width = Len::dip(w);
        l.height = Len::dip(h);
    })
}

fn responsive(n: NodeId, bounds: [f32; 2]) {
    Host::with(|h| {
        h.tree.set_flag(n, tree::RESPONSIVE, true);
        h.tree.author(n, |l| l.bounds = bounds);
    });
}

fn solve() {
    Host::with(|h| solve_root(h, root()));
}

fn geom(n: NodeId) -> Geom {
    Host::with(|h| h.tree.c.geom[n.index()])
}

fn rect(n: NodeId) -> Rect {
    geom(n).rect
}

// ── carried from the retired solver's own tests ─────────────────────────────────────

#[test]
fn snapping_keeps_adjacent_edges_exactly_shared() {
    for scale in [1.0_f32, 1.25, 1.5, 2.0] {
        let root = fixture(scale * 96.0);
        let column = node(root, Preset::Stack, |l| l.height = Len::dip(300.0));
        let a = sized(column, 100.0, 37.3);
        let b = sized(column, 100.0, 37.3);
        solve();
        assert_eq!(rect(a).y1, rect(b).y0, "edges parted at scale {scale}");
        let px = rect(a).y1 * scale;
        assert!((px - px.round()).abs() < 1.0e-3, "an edge missed the grid");
    }
}

#[test]
fn a_solve_places_children_in_order_and_absolutely() {
    let root = fixture(96.0);
    let row = node(root, Preset::Row, |l| l.height = Len::dip(40.0));
    let kids = [
        sized(row, 100.0, 20.0),
        sized(row, 100.0, 20.0),
        sized(row, 100.0, 20.0),
    ];
    solve();
    assert_eq!(rect(kids[0]).x0, 0.0);
    assert_eq!(rect(kids[1]).x0, 100.0);
    assert_eq!(rect(kids[2]).x0, 200.0);
    assert!(kids.iter().all(|k| (rect(*k).height() - 20.0).abs() < 0.01));
}

#[test]
fn a_hidden_node_keeps_its_slot_and_takes_no_space() {
    let root = fixture(96.0);
    let row = node(root, Preset::Row, |l| l.height = Len::dip(40.0));
    let kids = [
        sized(row, 100.0, 20.0),
        sized(row, 100.0, 20.0),
        sized(row, 100.0, 20.0),
    ];
    Host::with(|h| h.tree.set_flag(kids[1], tree::HIDDEN, true));
    solve();
    assert_eq!(rect(kids[2]).x0, 100.0);
    assert_eq!(geom(kids[1]).size, Vector2::zero());
    // Hidden, not removed: unhiding restores it with nothing rebuilt.
    Host::with(|h| h.tree.set_flag(kids[1], tree::HIDDEN, false));
    solve();
    assert_eq!(rect(kids[2]).x0, 200.0);
    assert_eq!(geom(kids[1]).size.x, 100.0);
}

#[test]
fn the_hidden_flag_is_independent_of_every_other_authored_field() {
    let root = fixture(96.0);
    let row = node(root, Preset::Row, |l| l.height = Len::dip(40.0));
    let grid = node(row, Preset::Grid, |l| {
        l.width = Len::dip(120.0);
        l.height = Len::dip(20.0);
    });
    Host::with(|h| {
        h.tree.set_flag(grid, tree::HIDDEN, true);
        // Re-stating the declaration while hidden neither reveals it nor rewrites what it is.
        h.tree.author(grid, |l| l.width = Len::dip(140.0));
    });
    solve();
    assert_eq!(geom(grid).size, Vector2::zero(), "a re-author revealed it");
    Host::with(|h| h.tree.set_flag(grid, tree::HIDDEN, false));
    solve();
    assert_eq!(geom(grid).size.x, 140.0);
    assert_eq!(
        Host::with(|h| h.tree.c.layout[grid.index()].preset),
        Preset::Grid,
        "hiding and showing rewrote what the node is"
    );
}

#[test]
fn becoming_responsive_keeps_the_declaration_and_the_children() {
    let root = fixture(96.0);
    let card = node(root, Preset::Row, |l| {
        l.width = Len::pct(1.0);
        l.height = Len::dip(40.0);
    });
    let kids = [
        sized(card, 100.0, 20.0),
        sized(card, 100.0, 20.0),
        sized(card, 100.0, 20.0),
    ];
    responsive(card, [600.0, 1000.0]);
    solve();
    assert_eq!(rect(kids[2]).x0, 200.0, "the children were dropped");
    assert_eq!(geom(card).size.x, 600.0, "the declaration was dropped");
}

#[test]
fn a_class_flip_re_measures_a_leaf_whose_own_inputs_did_not_change() {
    let root = fixture(96.0);
    let card = node(root, Preset::Stack, |l| l.width = Len::pct(1.0));
    responsive(card, [600.0, 1000.0]);
    // A fixed-width leaf: the same input at every width, so only re-measuring under the new
    // class can move it.
    let leaf = node(card, Preset::Layer, |l| {
        l.width = Len::dip(120.0);
        l.height = Len::times(Metric::Custom(&BY_CLASS), 1.0);
    });
    for (width, height) in [(1400.0, 20.0), (480.0, 60.0)] {
        window(width, 400.0);
        solve();
        assert_eq!(geom(leaf).size.y, height, "at {width} DIPs");
    }
}

#[test]
fn a_responsive_containers_first_class_never_floors_its_parent() {
    let root = fixture(96.0);
    // A fresh container's class is Wide until walk B hands it a width, so its first
    // measure runs the row as a row; the stacked sum must not reach the parent as a floor.
    let outer = node(root, Preset::Stack, |_| {});
    let card = node(outer, Preset::Stack, |_| {});
    responsive(card, [600.0, 1000.0]);
    let row = node(card, Preset::Row, |l| {
        l.stack_below = WidthClass::Narrow;
        l.gap = Len::ZERO;
    });
    sized(row, 436.0, 20.0);
    sized(row, 60.0, 20.0);
    window(200.0, 400.0);
    solve();
    assert_eq!(geom(outer).size.x, 200.0, "the row's pre-flip sum floored the parent");
    assert_eq!(geom(card).size.x, 200.0);
    assert_eq!(rect(row).height(), 40.0, "the row did not stack under Narrow");
}

#[test]
fn a_pixel_length_is_one_pixel_at_every_phase_where_a_dip_is_not() {
    let root = fixture(144.0);
    let scale = 1.5;
    // Rules stacked under spacers that put them at every phase of the 1.5 grid.
    let column = node(root, Preset::Stack, |l| l.gap = Len::ZERO);
    let mut rules = Vec::new();
    for lead in [0.0, 1.0, 2.0] {
        sized(column, 100.0, lead);
        rules.push((
            node(column, Preset::Layer, |l| l.height = Len::px(1.0)),
            node(column, Preset::Layer, |l| l.height = Len::dip(1.0)),
        ));
    }
    solve();
    let pixels = |n: NodeId| (rect(n).height() * scale).round();
    let mut dip_phases = Vec::new();
    for (px, dip) in rules {
        assert_eq!(pixels(px), 1.0, "a pixel rule at {:?}", rect(px));
        dip_phases.push(pixels(dip));
    }
    assert!(dip_phases.contains(&2.0), "a DIP hairline snapped to one pixel at every phase: {dip_phases:?}");
}

#[test]
fn a_node_minted_after_a_solve_reaches_the_next_one() {
    let root = fixture(96.0);
    let mid = node(root, Preset::Stack, |_| {});
    let group = node(mid, Preset::Stack, |_| {});
    solve();
    // Minted by a publication, which runs after that solve and takes its box from it.
    let line = sized(group, 47.0, 16.0);
    solve();
    assert_eq!(geom(line).size, Vector2::new(47.0, 16.0));
}

#[test]
fn a_detached_root_gathers_absolutely_at_its_origin() {
    let root = fixture(96.0);
    let row = node(root, Preset::Row, |l| l.height = Len::dip(40.0));
    let kid = sized(row, 100.0, 20.0);
    let overlay = Host::with(|h| {
        let id = h.tree.mint(0);
        h.tree.author(id, |l| *l = Layout::of(Preset::Stack));
        id
    });
    let item = sized(overlay, 120.0, 30.0);
    solve();
    Host::with(|h| solve_root(h, overlay));
    Host::with(|h| shift(h, overlay, Vector2::new(210.0, 64.0)));
    // The window subtree survived the second root.
    assert_eq!(rect(kid).x0, 0.0);
    assert_eq!(rect(overlay).x0, 210.0);
    assert_eq!(rect(overlay).y0, 64.0);
    assert_eq!(geom(overlay).size, Vector2::new(120.0, 30.0));
    // A root's offset within its parent is the origin it was placed at, so the placement
    // travels as the ordinary offset bind.
    assert_eq!(geom(overlay).local, Vector2::new(210.0, 64.0));
    assert_eq!(rect(item).x0, 210.0);
    assert_eq!(geom(item).local, Vector2::zero());
}

/// A window whose first child is a tenth of it, so the row under it sits at a fraction of
/// a pixel that the window height moves; the row holds one child centred in a box of odd
/// height, so its offset is a fraction too. Answers the row and the child.
fn fractional_column() -> [NodeId; 2] {
    let root = fixture(96.0);
    node(root, Preset::Layer, |l| l.height = Len::pct(0.1));
    let row = node(root, Preset::Row, |l| {
        l.width = Len::dip(200.0);
        l.height = Len::dip(40.0);
    });
    let child = sized(row, 100.0, 19.5);
    [row, child]
}

#[test]
fn a_resized_tree_lands_where_a_fresh_one_does() {
    let [_, child] = fractional_column();
    window(600.0, 373.0);
    solve();
    let fresh = rect(child);
    // The same tree, solved at another height first: the row's box keeps its size and
    // moves by a fraction, which is the walk that translates it rather than arranging it.
    let [_, child] = fractional_column();
    solve();
    Host::with(|h| h.tree.encode(&mut SinkPatch::default()));
    window(600.0, 373.0);
    solve();
    assert_eq!(rect(child), fresh, "the translate landed off the fresh box");
}

#[test]
fn the_offset_is_the_boxs_own_origin_against_its_parents() {
    let [row, child] = fractional_column();
    window(600.0, 373.0);
    solve();
    // The row sits at 37.3 and the child 10.25 inside it: the two fractions round apart, so
    // an offset snapped on its own would put the visual a pixel off the box the hit entry
    // and the clip describe.
    let expected = Vector2::new(
        rect(child).x0 - rect(row).x0,
        rect(child).y0 - rect(row).y0,
    );
    assert_eq!(geom(child).local, expected);
    assert_eq!(geom(child).local.y, 11.0);
}

/// A responsive container, its child, and a nested responsive container with a child.
fn nested() -> [NodeId; 4] {
    let root = fixture(96.0);
    let outer = node(root, Preset::Stack, |l| l.width = Len::pct(1.0));
    let child = node(outer, Preset::Stack, |l| l.height = Len::dip(10.0));
    // Fixed, so the nested container's own class is constant across the outer flip.
    let inner = node(outer, Preset::Stack, |l| l.width = Len::dip(300.0));
    let grandchild = node(inner, Preset::Stack, |l| l.height = Len::dip(10.0));
    responsive(outer, [600.0, 1000.0]);
    responsive(inner, [200.0, 400.0]);
    [outer, child, inner, grandchild]
}

#[test]
fn a_nodes_class_is_the_one_it_sits_in() {
    let [outer, child, inner, grandchild] = nested();
    window(300.0, 400.0);
    solve();
    let class = |n: NodeId| Host::with(|h| h.tree.class(n));
    // A container classifies its size for its subtree, so it reports the class it sits in.
    assert_eq!(class(outer), WidthClass::default());
    // 300 DIPs is Narrow against [600, 1000], and that governs the subtree.
    assert_eq!(class(child), WidthClass::Narrow);
    assert_eq!(class(inner), WidthClass::Narrow);
    // The nested one is 300 wide against [200, 400] — Medium — for its own subtree.
    assert_eq!(class(grandchild), WidthClass::Medium);
}

#[test]
fn a_flip_marks_the_subtree_and_stops_at_a_nested_container() {
    let [outer, child, inner, grandchild] = nested();
    for width in [1400.0_f32, 300.0] {
        window(width, 400.0);
        solve();
    }
    let marked = |n: NodeId| Host::with(|h| h.tree.c.flags[n.index()] & tree::MEASURE != 0);
    assert!(!marked(outer) && !marked(child) && !marked(inner) && !marked(grandchild));
    assert_eq!(
        Host::with(|h| h.tree.own_class(inner)),
        WidthClass::Medium,
        "the nested container did not keep its own class"
    );
    assert_eq!(Host::with(|h| h.tree.class(grandchild)), WidthClass::Medium);
}

// ── the shrink contract ─────────────────────────────────────────────────────────────

/// A 300-DIP row, to hang shrink and growth cases on.
fn shrink_row() -> NodeId {
    let root = fixture(96.0);
    node(root, Preset::Row, |l| {
        l.width = Len::dip(300.0);
        l.height = Len::dip(40.0);
    })
}

/// A child whose natural width is the sum of `boxes` and whose minimum is the widest one.
///
/// A stated size makes a leaf's minimum its natural, so an item that can give ground has to
/// be one whose own content breaks.
fn breakable(parent: NodeId, boxes: &[f32]) -> NodeId {
    let id = node(parent, Preset::Wrap, |_| {});
    for w in boxes {
        sized(id, *w, 20.0);
    }
    id
}

#[test]
fn naturals_that_fit_leave_the_surplus_to_the_growers_by_weight() {
    let row = shrink_row();
    let kids = [0.0, 1.0, 3.0].map(|grow| {
        node(row, Preset::Layer, |l| {
            l.width = Len::dip(60.0);
            l.height = Len::dip(20.0);
            l.grow = grow;
        })
    });
    solve();
    // 300 − 180 = 120 of surplus, shared one to three from each grower's natural basis.
    assert_eq!(geom(kids[0]).size.x, 60.0);
    assert_eq!(geom(kids[1]).size.x, 90.0);
    assert_eq!(geom(kids[2]).size.x, 150.0);
}

#[test]
fn a_misfit_gives_ground_proportionally_between_natural_and_minimum() {
    let row = shrink_row();
    let kids = [
        breakable(row, &[100.0, 100.0]),
        breakable(row, &[50.0, 50.0]),
        breakable(row, &[90.0, 10.0]),
    ];
    solve();
    // 400 natural against 300, so 100 of a 160 slack is given up, 5/8 of each item's own.
    // Published boxes are snapped to the pixel grid, so each share is asserted to the pixel.
    for (kid, share) in kids.iter().zip([137.5, 68.75, 93.75]) {
        assert!((geom(*kid).size.x - share).abs() <= 1.0, "a share was off by more than a pixel");
    }
}

#[test]
fn below_every_minimum_no_child_goes_under_its_own_and_the_residue_clips() {
    let row = shrink_row();
    // Natural 200, minimum 150, three times over in a 300-wide row.
    let kids = [0; 3].map(|_| breakable(row, &[150.0, 50.0]));
    solve();
    for kid in kids {
        assert_eq!(geom(kid).size.x, 150.0, "a child went under its minimum");
    }
    assert_eq!(geom(row).size.x, 300.0, "the container grew to its content");
}

#[test]
fn an_authored_minimum_only_raises_a_derived_floor() {
    let root = fixture(96.0);
    let stack = node(root, Preset::Stack, |l| {
        l.width = Len::dip(300.0);
        l.align = Align::Start;
    });
    let under = breakable(stack, &[50.0, 50.0]);
    let over = breakable(stack, &[50.0, 50.0]);
    Host::with(|h| {
        h.tree.author(under, |l| l.min_width = Len::dip(20.0));
        h.tree.author(over, |l| l.min_width = Len::dip(180.0));
    });
    solve();
    assert_eq!(
        geom(under).size.x,
        100.0,
        "a minimum under the content shrank it"
    );
    assert_eq!(
        geom(over).size.x,
        180.0,
        "a minimum over the content did not raise it"
    );
}

// ── layer, wrap and the ladder ──────────────────────────────────────────────────────

#[test]
fn a_layer_fills_a_child_that_states_no_size() {
    let root = fixture(96.0);
    let layer = node(root, Preset::Layer, |l| {
        l.width = Len::dip(200.0);
        l.height = Len::dip(100.0);
    });
    let fills = node(layer, Preset::Layer, |_| {});
    solve();
    assert_eq!(geom(fills).size, Vector2::new(200.0, 100.0));
}

#[test]
fn a_layer_aligns_a_child_that_states_one() {
    let root = fixture(96.0);
    let layer = node(root, Preset::Layer, |l| {
        l.width = Len::dip(200.0);
        l.height = Len::dip(100.0);
        l.align = Align::Center;
        l.justify = Align::Center;
    });
    let centred = sized(layer, 40.0, 20.0);
    solve();
    assert_eq!(geom(centred).local, Vector2::new(80.0, 40.0));
}

#[test]
fn an_all_band_layer_takes_its_authored_height() {
    let root = fixture(96.0);
    let layer = node(root, Preset::Layer, |l| {
        l.width = Len::dip(200.0);
        l.height = Len::dip(500.0);
    });
    let band = node(layer, Preset::Layer, |l| {
        l.position = Position::Band {
            at: Len::dip(120.0),
        };
        l.height = Len::dip(30.0);
    });
    solve();
    assert_eq!(
        geom(layer).size.y,
        500.0,
        "a band took room from its parent"
    );
    assert_eq!(geom(band).local.y, 120.0);
    assert_eq!(geom(band).size, Vector2::new(200.0, 30.0));
}

#[test]
fn a_wrap_breaks_and_stacks_its_lines_with_its_gap() {
    let root = fixture(96.0);
    let wrap = node(root, Preset::Wrap, |l| {
        l.width = Len::dip(220.0);
        l.gap = Len::dip(10.0);
        l.align = Align::Start;
    });
    let a = sized(wrap, 100.0, 20.0);
    let b = sized(wrap, 100.0, 20.0);
    let c = sized(wrap, 100.0, 20.0);
    solve();
    assert_eq!(geom(a).local.x, 0.0);
    assert_eq!(geom(b).local.x, 110.0);
    assert_eq!(
        geom(c).local,
        Vector2::new(0.0, 30.0),
        "the line did not break"
    );
    assert_eq!(
        geom(wrap).size.y,
        50.0,
        "the lines did not stack with the gap"
    );
}

#[test]
fn a_wrap_shrinks_a_lone_oversize_item() {
    let root = fixture(96.0);
    let wrap = node(root, Preset::Wrap, |l| l.width = Len::dip(120.0));
    // Natural 300, minimum 80: a stated size would be its own minimum and could not give ground.
    let lone = breakable(wrap, &[80.0, 80.0, 80.0, 60.0]);
    solve();
    assert_eq!(
        geom(lone).size.x,
        120.0,
        "a lone oversize item did not shrink"
    );
}

#[test]
fn a_ladder_yields_one_two_and_three_columns_without_remounting() {
    let root = fixture(96.0);
    let card = node(root, Preset::Grid, |l| {
        l.width = Len::pct(1.0);
        l.set_cols_by(&LADDER);
    });
    responsive(card, [600.0, 1000.0]);
    let first = node(card, Preset::Layer, |l| l.height = Len::dip(20.0));
    let _ = node(card, Preset::Layer, |l| l.height = Len::dip(20.0));
    let _ = node(card, Preset::Layer, |l| l.height = Len::dip(20.0));
    let minted = Host::with(|h| h.tree.ids.live());
    for (width, columns) in [(480.0_f32, 1.0_f32), (800.0, 2.0), (1400.0, 3.0)] {
        window(width, 400.0);
        solve();
        assert!(
            (geom(first).size.x - width / columns).abs() <= 1.0,
            "at {width} DIPs the ladder gave the wrong column count"
        );
    }
    assert_eq!(
        Host::with(|h| h.tree.ids.live()),
        minted,
        "a class change mounted or unmounted a node"
    );
}

/// Three weighted columns share a grid equally when their items state no minimum of their
/// own, whatever their content: a full-width item contributes nothing, and a short label is
/// below the share.
#[test]
fn equal_weights_give_equal_columns_over_unequal_content() {
    let root = fixture(96.0);
    let grid = node(root, Preset::Grid, |l| {
        l.width = Len::dip(191.0);
        l.gap = Len::dip(4.0);
        l.set_cols([Track::fr(1.0); 3]);
    });
    let mut cells = [NodeId::NONE; 3];
    for (k, w) in [30.0_f32, 20.0, 10.0].into_iter().enumerate() {
        let cell = node(grid, Preset::Stack, |l| l.min_width = Len::ZERO);
        let _ = node(cell, Preset::Layer, |l| {
            l.width = Len::pct(1.0);
            l.height = Len::dip(20.0);
        });
        let _ = node(cell, Preset::Layer, |l| {
            l.width = Len::dip(w);
            l.height = Len::dip(10.0);
        });
        cells[k] = cell;
    }
    solve();
    let widths: Vec<f32> = cells.iter().map(|&c| geom(c).size.x).collect();
    assert!(
        widths.iter().all(|&w| (w - 61.0).abs() <= 1.0),
        "columns are not equal: {widths:?}"
    );
}

/// A row template sizes the rows: a bounded row takes its share up to its maximum and never
/// less than its minimum, and a weighted row takes what is left. A grid stating no columns
/// has one that fills it.
#[test]
fn a_row_template_bounds_one_row_and_gives_the_rest_to_the_weighted_one() {
    let root = fixture(96.0);
    let grid = node(root, Preset::Grid, |l| {
        l.width = Len::pct(1.0);
        l.height = Len::pct(1.0);
        l.set_rows([Track::bounded(Len::dip(100.0), Len::pct(0.25)), Track::fr(1.0)]);
    });
    let top = node(grid, Preset::Layer, |_| {});
    let rest = node(grid, Preset::Layer, |_| {});
    for (height, first) in [(800.0_f32, 200.0_f32), (300.0, 100.0)] {
        window(600.0, height);
        solve();
        assert!((geom(top).size.y - first).abs() <= 1.0, "at {height}: {:?}", geom(top).size);
        assert!((geom(rest).size.y - (height - first)).abs() <= 1.0, "at {height}: {:?}", geom(rest).size);
        assert!((geom(rest).size.x - 600.0).abs() <= 1.0, "the lone column did not fill the grid");
    }
}

// ── the dirty frontier ──────────────────────────────────────────────────────────────

#[test]
fn a_resize_visits_no_clean_subtree() {
    let root = fixture(96.0);
    let fixed = node(root, Preset::Stack, |l| {
        l.width = Len::dip(200.0);
        l.height = Len::dip(200.0);
    });
    for _ in 0..8 {
        node(fixed, Preset::Layer, |l| l.height = Len::dip(10.0));
    }
    let grown = node(root, Preset::Stack, |l| {
        l.width = Len::pct(1.0);
        l.height = Len::dip(50.0);
    });
    solve();
    take_visits();
    window(900.0, 400.0);
    solve();
    let visited = take_visits();
    // The root and the child whose width moved; the fixed one's assigned width did not, so
    // walk B returned there and its eight children were never reached.
    assert!(
        visited <= 3,
        "a resize descended into a clean subtree: {visited} nodes placed"
    );
    assert_eq!(geom(grown).size.x, 900.0);
}

#[test]
fn the_walks_ask_the_text_table_once_per_class_and_width() {
    let root = fixture(96.0);
    let column = node(root, Preset::Stack, |l| l.width = Len::pct(1.0));
    let _leaf = node(column, Preset::Text, |l| l.height = Len::dip(16.0));
    solve();
    for round in 0..5 {
        let width = if round % 2 == 0 { 600.0 } else { 400.0 };
        window(width, 400.0);
        take_asks();
        solve();
        let asks = take_asks();
        assert!(
            asks <= 2,
            "solve {round} asked the text table {asks} times for one leaf"
        );
    }
}

#[test]
fn a_flush_solves_each_root_once() {
    let root = fixture(96.0);
    let _ = sized(root, 100.0, 20.0);
    let mut patch = SinkPatch::default();
    Host::flush(&mut patch);
    patch.clear();
    take_roots();
    Host::with(|h| h.tree.author(root, |l| l.height = Len::dip(400.0)));
    Host::flush(&mut patch);
    assert_eq!(
        take_roots(),
        1,
        "one flush ran more than one solve per root"
    );
    assert!(
        Host::with(|h| h.tree.unsettled().is_none()),
        "a publisher left a layout input unsolved"
    );
}

// ── zero allocation ─────────────────────────────────────────────────────────────────

/// Flushes twice under `edit` and answers what the second one allocated.
fn warm_flush(patch: &mut SinkPatch, mut edit: impl FnMut()) -> usize {
    for _ in 0..2 {
        edit();
        Host::flush(patch);
        patch.clear();
    }
    edit();
    let before = crate::counting::allocations();
    Host::flush(patch);
    let spent = crate::counting::allocations() - before;
    patch.clear();
    spent
}

#[test]
fn a_warm_resize_allocates_nothing() {
    let root = fixture(96.0);
    let stack = node(root, Preset::Stack, |l| l.width = Len::pct(1.0));
    for _ in 0..8 {
        node(stack, Preset::Layer, |l| l.height = Len::dip(10.0));
    }
    let mut patch = SinkPatch::default();
    Host::flush(&mut patch);
    patch.clear();
    let mut step = 0u32;
    let spent = warm_flush(&mut patch, || {
        step += 1;
        window(
            900.0 + f32::from(u16::try_from(step % 2).unwrap_or(0)),
            400.0,
        );
    });
    assert_eq!(spent, 0, "a warm resize allocated {spent} times");
}

#[test]
fn a_warm_class_flip_allocates_nothing() {
    let root = fixture(96.0);
    let card = node(root, Preset::Stack, |l| l.width = Len::pct(1.0));
    responsive(card, [600.0, 1000.0]);
    let leaf = node(card, Preset::Layer, |l| {
        l.width = Len::dip(120.0);
        l.height = Len::times(Metric::Custom(&BY_CLASS), 1.0);
    });
    let mut patch = SinkPatch::default();
    Host::flush(&mut patch);
    patch.clear();
    let mut step = 0u32;
    let spent = warm_flush(&mut patch, || {
        step += 1;
        window(if step % 2 == 0 { 480.0 } else { 1400.0 }, 400.0);
    });
    assert_eq!(spent, 0, "a warm class flip allocated {spent} times");
    assert!(geom(leaf).size.y > 0.0);
}

// ── bytes ───────────────────────────────────────────────────────────────────────────

#[test]
fn the_per_node_rows_are_the_size_the_arena_budgets_for() {
    assert_eq!(size_of::<Len>(), 8);
    assert!(size_of::<Layout>() <= 128, "{}", size_of::<Layout>());
    assert_eq!(size_of::<Geom>(), 64);
}

#[test]
fn snapping_a_non_finite_coordinate_answers_the_origin() {
    assert_eq!(snap(f32::NAN, 1.5), 0.0);
    assert_eq!(snap(f32::INFINITY, 1.5), 0.0);
}
