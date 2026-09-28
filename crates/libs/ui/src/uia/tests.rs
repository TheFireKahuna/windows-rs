//! Headless tests for the automation tree: what a publish builds, what a query resolves,
//! and what each path allocates.
//!
//! A query resolves against a published snapshot, so nothing here needs a compositor, a
//! message pump or a COM apartment.

use super::snapshot::FieldText;
use super::*;
use crate::counting::allocations;
use crate::widget::{Range, UiaRole};
use core::sync::atomic::Ordering::Relaxed;
use windows_numerics::Vector2;
use windows_scene::{
    ContactKind, ControlId, HitEntry, HitFlags, HitTable, NO_ENTRY, NodeId, Point,
};

/// A screen under construction: the automation rows, the hit array the same walk would fill,
/// and the authority that minted their control ids.
///
/// Both tables are filled by one call, because the host fills them in one walk: a fixture that
/// built them apart could not tell whether the two agree.
pub(super) struct Screen {
    pub(super) snapshot: Snapshot,
    pub(super) entries: Vec<HitEntry>,
    /// The authority the stack itself uses, so an id under test is dense from one and
    /// generational, and never collides with the root's `ControlId::NONE`.
    authority: windows_scene::Ids<{ windows_scene::CONTROL }>,
    minted: Vec<ControlId>,
}

impl Screen {
    /// Returns an empty screen whose id authority has minted nothing.
    pub(super) fn new() -> Self {
        Self {
            snapshot: Snapshot::default(),
            entries: Vec::new(),
            authority: windows_scene::Ids::default(),
            minted: Vec::new(),
        }
    }

    /// Returns an empty screen that continues this one's id authority, releasing every id
    /// this one minted.
    ///
    /// The release is what makes those ids stale. Ids are dense, so a second authority
    /// would hand the replacement screen the same ids the first one used and nothing would
    /// go stale at all.
    pub(super) fn successor(mut self) -> Self {
        for id in self.minted.drain(..) {
            self.authority.release(id);
        }
        Self {
            snapshot: Snapshot::default(),
            entries: Vec::new(),
            authority: self.authority,
            minted: Vec::new(),
        }
    }

    /// Returns the control id of the `index`th element added to this screen.
    pub(super) fn control(&self, index: u16) -> ControlId {
        self.minted[index as usize]
    }

    /// Adds an element under `parent` and returns its index in both tables.
    ///
    /// `parent` is an index, or [`NONE`] for a top-level element. The element is focusable and
    /// enabled, carries no value, and takes its id from this screen's authority.
    pub(super) fn add(
        &mut self,
        parent: u16,
        rect: (f32, f32, f32, f32),
        role: UiaRole,
        name: &str,
    ) -> u16 {
        let at = self.entries.len() as u16;
        let id = self.authority.mint();
        self.minted.push(id);
        let name = self.snapshot.intern(name);
        self.snapshot.entries.push(Entry {
            id,
            box_: [rect.0, rect.1, rect.2, rect.3],
            name,
            parent,
            child: NONE,
            next: NONE,
            clip: NONE,
            scroll: NONE,
            flags: ColFlags::FOCUSABLE,
            role,
        });
        self.snapshot.state.push(State::ENABLED);
        self.entries.push(HitEntry {
            x0: rect.0,
            y0: rect.1,
            x1: rect.2,
            y1: rect.3,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: if parent == NONE {
                NO_ENTRY
            } else {
                u32::from(parent)
            },
            flags: HitFlags::INTERACTIVE | HitFlags::UIA,
            scroll_src: NodeId::NONE,
            id,
        });
        at
    }

    /// Adds a slider named `gain` under `parent`, bounded by `range`, and returns its index.
    pub(super) fn slider(&mut self, parent: u16, rect: (f32, f32, f32, f32), range: Range) -> u16 {
        let at = self.add(parent, rect, UiaRole::Slider, "gain");
        self.snapshot.ranges.push((at, range));
        self.snapshot.entries[at as usize].flags =
            self.snapshot.entries[at as usize].flags | ColFlags::RANGED;
        at
    }

    /// Adds an editable field under `parent` and returns its index.
    ///
    /// `text` is shaped into one cluster per character, each `advance` wide and as tall as the
    /// field, so a supplementary character is one cluster over two code units and a cluster
    /// walk is distinguishable from a code-unit walk. The field's viewport is its own box, so
    /// nothing is cut by the reveal.
    pub(super) fn field(
        &mut self,
        parent: u16,
        rect: (f32, f32, f32, f32),
        text: &str,
        advance: f32,
    ) -> u16 {
        let at = self.add(parent, rect, UiaRole::Edit, "value");
        let units: Vec<u16> = text.encode_utf16().collect();
        let height = rect.3 - rect.1;
        let mut clusters = Vec::new();
        let mut start = 0u32;
        for (index, ch) in text.chars().enumerate() {
            let end = start + ch.len_utf16() as u32;
            let x = index as f32 * advance;
            clusters.push(crate::text_input::Cluster {
                start,
                end,
                rect: windows_text::Rect {
                    x,
                    y: 0.0,
                    w: advance,
                    h: height,
                },
                leading: x,
                trailing: x + advance,
            });
            start = end;
        }
        self.snapshot.entries[at as usize].flags =
            self.snapshot.entries[at as usize].flags | ColFlags::BODY | ColFlags::FIELD;
        self.snapshot.fields.push(FieldText {
            id: self.minted[at as usize],
            revision: 1,
            text: units.into(),
            selection: crate::text_input::Selection::default(),
            geometry: Some(Arc::new(crate::text_input::Geometry {
                revision: 1,
                clusters: clusters.into(),
                viewport: windows_text::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: rect.2 - rect.0,
                    h: height,
                },
                ..crate::text_input::Geometry::default()
            })),
            password: false,
        });
        at
    }

    /// Replaces the published text of the field at `at`, advancing its revision.
    pub(super) fn retype(&mut self, at: u16, text: &str) {
        let id = self.minted[at as usize];
        let units: Vec<u16> = text.encode_utf16().collect();
        if let Some(field) = self.snapshot.fields.iter_mut().find(|f| f.id == id) {
            field.revision += 1;
            field.text = units.into();
            if let Some(geometry) = field.geometry.as_mut() {
                Arc::make_mut(geometry).revision = field.revision;
            }
        }
    }

    /// Moves the published selection of the field at `at`.
    pub(super) fn reselect(&mut self, at: u16, anchor: u32, caret: u32) {
        let id = self.minted[at as usize];
        if let Some(field) = self.snapshot.fields.iter_mut().find(|f| f.id == id) {
            field.selection = crate::text_input::Selection {
                anchor,
                caret,
                ..crate::text_input::Selection::default()
            };
        }
    }

    /// Records that the elements in `rows` are clipped by `container` and scroll with `node`.
    pub(super) fn scrolls(&mut self, container: u16, node: NodeId, rows: &[u16]) {
        let row = self.snapshot.scrolls.len() as u16;
        self.snapshot.scrolls.push(ScrollView::new(node, container));
        self.entries[container as usize].flags =
            self.entries[container as usize].flags | HitFlags::SCROLL | HitFlags::CLIP;
        for &at in rows {
            self.snapshot.entries[at as usize].clip = container;
            self.snapshot.entries[at as usize].scroll = row;
            self.entries[at as usize].clip_parent = u32::from(container);
            self.entries[at as usize].scroll_src = node;
        }
    }

    /// Publishes this screen's rows to `uia`.
    pub(super) fn publish(&mut self, uia: &mut Uia) {
        uia.publish(&self.snapshot);
    }

    /// Returns the hit table the pointer would route through, over the same walk's entries.
    pub(super) fn table(&self) -> HitTable {
        let mut index: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .map(|(at, entry)| (entry.id, at as u32))
            .collect();
        index.sort_unstable_by_key(|&(id, _)| id);
        let mut table = HitTable::default();
        table.replace(&self.entries, &index);
        table
    }
}

/// Returns a [`Uia`] latched as though a client had attached, so a publish builds a tree.
///
/// The window origin is at zero and its scale is 1, which keeps a control's bounds equal to
/// the rect it was laid out with.
pub(super) fn listening() -> Uia {
    let mut uia = Uia::new();
    uia.latch_for_test();
    uia.set_window(Vector2 { x: 0.0, y: 0.0 }, 1.0);
    uia
}

#[test]
fn nothing_is_built_until_something_is_listening() {
    let mut uia = Uia::new();
    let mut screen = Screen::new();
    screen.add(NONE, (0.0, 0.0, 100.0, 40.0), UiaRole::Button, "mute");
    screen.publish(&mut uia);

    assert!(
        uia.tree().is_empty(),
        "an unattached machine pays for no tree at all"
    );
    uia.latch_for_test();
    screen.publish(&mut uia);
    assert_eq!(
        uia.tree().entries().len(),
        1,
        "and the latch is what starts it"
    );
}

#[test]
fn a_published_tree_carries_its_names_and_its_shape() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.add(group, (96.0, 8.0, 168.0, 32.0), UiaRole::Button, "solo");
    screen.publish(&mut uia);

    let tree = uia.tree();
    assert_eq!(tree.entries().len(), 3);
    let read = |at: u16| String::from_utf16_lossy(tree.text(tree.at(at).unwrap().name));
    assert_eq!(read(0), "output");
    assert_eq!(read(1), "mute");
    assert_eq!(tree.at(1).unwrap().parent, 0);
    assert_eq!(tree.at(0).unwrap().child, 1);
    assert_eq!(tree.last_child(0), 2);
    assert_eq!(tree.at(1).unwrap().next, 2);
}

/// Element-from-point and the pointer's hit test resolve over two tables the same walk fills,
/// so the two answer identically at every point.
#[test]
fn element_from_point_agrees_with_the_pointer_over_ten_thousand_points() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let card = screen.add(NONE, (0.0, 0.0, 300.0, 200.0), UiaRole::Group, "card");
    screen.add(card, (10.0, 10.0, 90.0, 40.0), UiaRole::Button, "a");
    screen.add(card, (100.0, 10.0, 180.0, 40.0), UiaRole::Button, "b");
    screen.add(card, (10.0, 60.0, 180.0, 90.0), UiaRole::Slider, "c");
    screen.publish(&mut uia);

    let table = screen.table();
    let tree = uia.tree();
    // A fixed xorshift seed, so the sweep covers the same points on every run. The range
    // overhangs the card on all four sides, so misses are covered too.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..10_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let p = Point {
            x: f32::from((state >> 32) as u16) / 65_535.0 * 320.0 - 10.0,
            y: f32::from(state as u16) / 65_535.0 * 220.0 - 10.0,
        };
        let pointer = table.hit(p, ContactKind::Mouse).map(|hit| hit.id);
        let automation = tree.hit(p).map(|at| tree.at(at).unwrap().id);
        assert_eq!(pointer, automation, "the two disagreed at {p:?}");
    }
}

/// The offset moves the point and not the rects, so a scrolled row is found where it is drawn
/// and the pointer agrees.
#[test]
fn a_scrolled_row_is_found_where_it_is_drawn_and_not_where_it_was_laid_out() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let list = screen.add(NONE, (0.0, 0.0, 200.0, 100.0), UiaRole::List, "presets");
    let track = NodeId::FIRST;
    let first = screen.add(list, (0.0, 0.0, 200.0, 50.0), UiaRole::Button, "flat");
    let second = screen.add(list, (0.0, 50.0, 200.0, 100.0), UiaRole::Button, "vocal");
    screen.scrolls(list, track, &[first, second]);
    screen.publish(&mut uia);

    let under = |uia: &Uia, y: f32| {
        let tree = uia.tree();
        let at = tree.hit(Point { x: 100.0, y })?;
        Some(String::from_utf16_lossy(tree.text(tree.at(at)?.name)))
    };
    assert_eq!(under(&uia, 25.0).as_deref(), Some("flat"));

    // The list scrolls by one row. The rects do not move; the offset does.
    uia.set_scroll(track, Vector2 { x: 0.0, y: 50.0 });
    assert_eq!(
        under(&uia, 25.0).as_deref(),
        Some("vocal"),
        "the second row is what is drawn at the top now"
    );
}

#[test]
fn a_republish_carries_state_forward_rather_than_resetting_it() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let toggle = screen.add(NONE, (0.0, 0.0, 80.0, 32.0), UiaRole::CheckBox, "bypass");
    let slider = screen.slider(NONE, (0.0, 40.0, 200.0, 64.0), Range::new(-24.0, 24.0));
    screen.publish(&mut uia);

    let (toggle_id, slider_id) = (screen.control(toggle), screen.control(slider));
    uia.set_state(toggle_id, State::TOGGLED, true);
    uia.set_value(slider_id, -6.0);

    // A resize: the same controls in different boxes, with one new element ahead of them so
    // their indices move. The rows carry their own ids, so nothing is re-interned by hand.
    let mut resized = Screen::new();
    resized.add(NONE, (0.0, 0.0, 200.0, 24.0), UiaRole::Text, "output");
    for (id, rect) in [
        (toggle_id, (0.0, 30.0, 90.0, 62.0)),
        (slider_id, (0.0, 70.0, 220.0, 94.0)),
    ] {
        let at = resized.add(NONE, rect, UiaRole::CheckBox, "carried");
        resized.snapshot.entries[at as usize].id = id;
        resized.entries[at as usize].id = id;
        // The walk derives model state from the application's own rows, so a publish always
        // states it; the fixture states what a walk over an unchanged model would.
        if id == toggle_id {
            resized.snapshot.state[at as usize] = State::ENABLED | State::TOGGLED;
        }
    }
    resized.publish(&mut uia);

    let tree = uia.tree();
    let toggle_at = tree.index_of(toggle_id).expect("still mounted");
    let slider_at = tree.index_of(slider_id).expect("still mounted");
    assert!(
        tree.state(toggle_at).has(State::TOGGLED),
        "a resize is not a reset"
    );
    assert_eq!(
        tree.value(slider_at),
        Some(-6.0),
        "a value the publish left unstated was dropped"
    );
    assert_ne!(toggle_at, 0, "and the indices really did move");
}

/// What the publish states about model state replaces what the tree before it held.
///
/// The walk derives enabled, toggled, selected and expanded from the application's own rows,
/// so a carry would report a switch at the position the user has just flipped it from.
#[test]
fn a_publish_states_model_state_rather_than_inheriting_it() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let toggle = screen.add(NONE, (0.0, 0.0, 80.0, 32.0), UiaRole::CheckBox, "bypass");
    screen.snapshot.state[toggle as usize] = State::ENABLED | State::TOGGLED;
    screen.publish(&mut uia);
    let id = screen.control(toggle);
    assert!(uia.tree().state(0).has(State::TOGGLED));

    let mut next = screen.successor();
    let at = next.add(NONE, (0.0, 0.0, 80.0, 32.0), UiaRole::CheckBox, "bypass");
    next.snapshot.entries[at as usize].id = id;
    next.entries[at as usize].id = id;
    next.publish(&mut uia);
    assert!(
        !uia.tree().state(0).has(State::TOGGLED),
        "the tree reported the switch at the position it was flipped from"
    );
}

#[test]
fn an_absent_value_is_absent_and_a_range_reports_its_bounds() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let slider = screen.slider(NONE, (0.0, 0.0, 200.0, 24.0), Range::new(-60.0, 0.0));
    let button = screen.add(NONE, (0.0, 30.0, 80.0, 62.0), UiaRole::Button, "reset");
    screen.publish(&mut uia);

    let at = uia.tree().index_of(screen.control(slider)).unwrap();
    assert_eq!(uia.tree().value(at), None, "unwritten is absent, not zero");
    uia.set_value(screen.control(slider), 0.0);
    assert_eq!(uia.tree().value(at), Some(0.0), "and zero is a real value");

    let tree = uia.tree();
    let range = tree.range(at).expect("the slider declared bounds");
    assert_eq!((range.min, range.max), (-60.0, 0.0));
    let at = tree.index_of(screen.control(button)).unwrap();
    assert_eq!(tree.range(at), None);
}

#[test]
fn patterns_follow_the_role_and_what_the_element_declared() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let plain = screen.add(NONE, (0.0, 0.0, 80.0, 32.0), UiaRole::Button, "reset");
    let opener = screen.add(NONE, (0.0, 40.0, 80.0, 72.0), UiaRole::Button, "mode");
    screen.snapshot.entries[opener as usize].flags = ColFlags::FOCUSABLE | ColFlags::EXPANDS;
    screen.publish(&mut uia);

    let tree = uia.tree();
    let at = |index: u16| tree.index_of(screen.control(index)).unwrap();
    assert!(tree.patterns(at(plain)).has(Patterns::INVOKE));
    assert!(
        !tree.patterns(at(plain)).has(Patterns::EXPAND),
        "a button that opens nothing does not answer expand-collapse"
    );
    assert!(tree.patterns(at(opener)).has(Patterns::EXPAND));
}

#[test]
fn a_queued_action_is_taken_once_and_the_queue_stays_bounded() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let button = screen.add(NONE, (0.0, 0.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.publish(&mut uia);

    let id = screen.control(button);
    uia.queue_for_test(Action::Invoke(id));
    uia.queue_for_test(Action::SetValue(id, 1.0));
    uia.queue_for_test(Action::SetValue(id, 2.0));

    let mut out = Vec::new();
    uia.drain(&mut out);
    assert_eq!(
        out,
        [Action::Invoke(id), Action::SetValue(id, 2.0)],
        "a repeated set-value supersedes; an invoke does not"
    );
    uia.drain(&mut out);
    assert_eq!(out.len(), 2, "and a drained queue yields nothing more");
}

#[test]
fn a_live_region_announces_a_change_and_not_a_heartbeat() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let meter = screen.slider(NONE, (0.0, 0.0, 200.0, 24.0), Range::new(-60.0, 0.0));
    screen.snapshot.entries[meter as usize].flags = ColFlags::LIVE_POLITE | ColFlags::RANGED;
    screen.publish(&mut uia);
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised);
    raised.clear();

    // A producer at display rate, drifting by less than one announcement quantum in total.
    for step in 0..64 {
        uia.set_value(screen.control(meter), -14.0 + f64::from(step) * 0.001);
    }
    uia.take_pending_for_test(&mut raised);
    let announced = |raised: &[Raise]| {
        raised
            .iter()
            .filter(|raise| **raise == Raise::live(screen.control(meter)))
            .count()
    };
    assert_eq!(announced(&raised), 1, "one announcement, not sixty-four");
    raised.clear();

    uia.set_value(screen.control(meter), -3.0);
    uia.take_pending_for_test(&mut raised);
    assert_eq!(announced(&raised), 1, "and a real move does announce");
}

#[test]
fn a_region_part_is_an_element_the_scan_can_reach() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let region = screen.add(NONE, (0.0, 0.0, 400.0, 200.0), UiaRole::Graph, "spectrum");
    let id = screen.control(region);
    uia.set_parts(
        id,
        &[
            Part {
                sub: 0,
                name: "band 1",
                role: UiaRole::Slider,
                rect: [10.0, 10.0, 30.0, 190.0],
            },
            Part {
                sub: 1,
                name: "band 2",
                role: UiaRole::Slider,
                rect: [40.0, 10.0, 60.0, 190.0],
            },
        ],
    );
    screen.publish(&mut uia);

    let tree = uia.tree();
    let at = tree.index_of(id).unwrap();
    assert_eq!(uia.parts_for_test(id).0, 2);

    // The region's own entry is what the scan finds; the part is resolved inside it, which is
    // the same order pointer routing uses.
    let found = tree.hit(Point { x: 50.0, y: 100.0 });
    assert_eq!(found, Some(at), "the region is under the point");
    let entry = tree.at(at).unwrap();
    assert_eq!(
        uia.part_at_for_test(id, 50.0 - entry.box_[0], 100.0 - entry.box_[1]),
        Some(1),
        "the second band is under the point, inside the region that won the scan"
    );
}

#[test]
fn releasing_a_control_forgets_what_it_declared() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let region = screen.add(NONE, (0.0, 0.0, 400.0, 200.0), UiaRole::Graph, "spectrum");
    let id = screen.control(region);
    uia.set_parts(
        id,
        &[Part {
            sub: 0,
            name: "band 1",
            role: UiaRole::Slider,
            rect: [10.0, 10.0, 30.0, 190.0],
        }],
    );
    screen.publish(&mut uia);
    assert_eq!(uia.parts_for_test(id).0, 1);

    uia.release(id);
    assert_eq!(
        uia.parts_for_test(id).0,
        0,
        "a released control leaves nothing behind, and needs no republish to say so"
    );
}

// ── allocation cost ─────────────────────────────────────────────────────────────
//
// These count allocations rather than checking capacity: a temporary allocated and freed
// inside a call leaves every capacity where it was, so only a count sees it.

#[test]
fn the_interaction_path_allocates_nothing() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let slider = screen.slider(NONE, (0.0, 0.0, 200.0, 24.0), Range::new(-60.0, 0.0));
    let toggle = screen.add(NONE, (0.0, 30.0, 80.0, 62.0), UiaRole::CheckBox, "bypass");
    screen.publish(&mut uia);
    // Warm-up, so the pending set reaches its high-water mark before the count starts and
    // the loop below measures a steady drag rather than the first event of one.
    let mut raised = Vec::new();
    uia.set_value(screen.control(slider), -1.0);
    uia.set_state(screen.control(toggle), State::TOGGLED, true);
    uia.set_focus(Some(screen.control(toggle)));
    uia.take_pending_for_test(&mut raised);
    raised.clear();

    let before = allocations();
    for step in 0..256 {
        uia.set_value(screen.control(slider), f64::from(step) * -0.25);
        uia.set_scroll(NodeId::NONE, Vector2 { x: 0.0, y: 4.0 });
        uia.set_window(Vector2 { x: 12.0, y: 34.0 }, 1.5);
    }
    uia.set_state(screen.control(toggle), State::TOGGLED, true);
    uia.set_focus(Some(screen.control(slider)));
    assert_eq!(
        allocations() - before,
        0,
        "a drag is a relaxed store per sample and a folded event, and neither allocates"
    );
}

#[test]
fn a_query_allocates_only_what_com_demands() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.publish(&mut uia);
    let root = uia.root_for_test();
    // The first ask mints the provider object and this thread's reference to the tree.
    // Minting costs one row per element a client visits, and nothing for a session that
    // queried none; the loop below measures the walk a client then repeats.
    let mint = allocations();
    let first = provider::provider_for(&uia.shared, screen.control(1));
    let mint = allocations() - mint;
    // Both elements minted, so what follows is a client re-walking a tree it has seen.
    drop(provider::provider_for(&uia.shared, screen.control(0)));

    let before = allocations();
    for _ in 0..64 {
        drop(provider::provider_for(&uia.shared, screen.control(1)));
        drop(provider::provider_for(&uia.shared, screen.control(0)));
    }
    assert_eq!(
        allocations() - before,
        0,
        "resolving a published element reads a snapshot and an interned object: no copy and \
         no allocation, however many times a client asks"
    );
    assert!(mint <= 3, "and minting one costs {mint}, not a tree walk");
    drop(first);
    drop(root);
}

/// Region parts live beside the published tree, not in it, so a renderer moving its part
/// geometry republishes no element and raises no structure-changed event.
#[test]
fn a_moving_region_changes_its_parts_and_not_the_tree() {
    use std::sync::Arc;
    use windows_present::{Rect, RegionParts, SubId};

    let mut uia = listening();
    let mut screen = Screen::new();
    let region = screen.add(NONE, (0.0, 0.0, 400.0, 200.0), UiaRole::Graph, "spectrum");
    screen.publish(&mut uia);
    let id = screen.control(region);

    let geometry = Arc::new(RegionParts::new());
    uia.watch_region(RegionPeer {
        format: None,
        updates: None,
        id,
        geometry: Arc::clone(&geometry),
        parts: vec![PartDecl::new(0, "Low band", UiaRole::Slider)],
        values: None,
        value: None,
    });
    let publish_at = |x: f32| {
        geometry.publish(&[windows_present::Part {
            id: SubId(0),
            rect: Rect::new(x, 0.0, x + 20.0, 100.0),
        }]);
    };

    publish_at(10.0);
    uia.sync_regions();
    let before = uia.tree_arc_for_test();
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised);
    raised.clear();

    publish_at(90.0);
    uia.sync_regions();

    assert_eq!(
        uia.part_at_for_test(id, 95.0, 50.0),
        Some(0),
        "the band is where the renderer just put it"
    );
    assert!(
        Arc::ptr_eq(&before, &uia.tree_arc_for_test()),
        "and the tree is the same tree — the mapping moved, the structure did not"
    );
    uia.take_pending_for_test(&mut raised);
    assert!(
        !raised.iter().any(|r| matches!(r, Raise::Structure(..))),
        "so no client is told the window was rebuilt"
    );
}

/// A dragged band republishes its part geometry every frame, so the join that picks it up
/// runs per frame and allocates on neither side of the hand-off.
#[test]
fn re_joining_a_moving_region_allocates_nothing() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use windows_present::{Rect, RegionParts, SubId};

    let mut uia = listening();
    let mut screen = Screen::new();
    let region = screen.add(NONE, (0.0, 0.0, 400.0, 200.0), UiaRole::Graph, "spectrum");
    screen.publish(&mut uia);
    let id = screen.control(region);

    let geometry = Arc::new(RegionParts::new());
    let levels: Arc<[AtomicU64]> = Arc::from([AtomicU64::new(0), AtomicU64::new(0)]);
    uia.watch_region(RegionPeer {
        format: None,
        updates: None,
        id,
        geometry: Arc::clone(&geometry),
        parts: vec![
            PartDecl::new(0, "Low band", UiaRole::Slider),
            PartDecl::new(1, "Mid band", UiaRole::Slider),
        ],
        values: Some(Arc::clone(&levels)),
        value: None,
    });
    let publish_at = |x: f32| {
        geometry.publish(&[
            windows_present::Part {
                id: SubId(0),
                rect: Rect::new(x, 0.0, x + 20.0, 100.0),
            },
            windows_present::Part {
                id: SubId(1),
                rect: Rect::new(x + 40.0, 0.0, x + 60.0, 100.0),
            },
        ]);
    };
    // Warm-up, so every buffer on both sides of the hand-off reaches its high-water mark
    // before the count starts.
    for step in 0..4 {
        publish_at(step as f32);
        uia.sync_regions();
        let _ = uia.parts_for_test(id);
    }

    let before = allocations();
    for step in 0..256 {
        levels[0].store(f64::from(step).to_bits(), Relaxed);
        publish_at(step as f32);
        uia.sync_regions();
    }
    assert_eq!(
        allocations() - before,
        0,
        "a drag joins into buffers it already has, on both sides of the publish"
    );

    // A tick where the renderer has published nothing new is one version load per watched
    // region, and copies no parts.
    let before = allocations();
    for _ in 0..256 {
        uia.sync_regions();
    }
    assert_eq!(allocations() - before, 0);
    assert_eq!(uia.parts_for_test(id), (2, Some(0.0)));
}

#[test]
fn a_publish_allocates_a_bounded_amount_and_an_idle_window_allocates_none() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let card = screen.add(NONE, (0.0, 0.0, 300.0, 200.0), UiaRole::Group, "card");
    for _ in 0..32 {
        screen.add(card, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "row");
    }
    screen.publish(&mut uia);

    let before = allocations();
    screen.publish(&mut uia);
    let cost = allocations() - before;
    assert!(
        cost <= 24,
        "a republish of 33 elements should cost a handful of allocations, not one per \
         element; it cost {cost}"
    );

    // Repeating unchanged window geometry queues nothing.
    uia.set_window(Vector2 { x: 1.0, y: 2.0 }, 1.0);
    let before = allocations();
    for _ in 0..64 {
        uia.set_window(Vector2 { x: 1.0, y: 2.0 }, 1.0);
    }
    assert_eq!(allocations() - before, 0);
}

/// Text and selection changes preserve fragment topology.
#[test]
fn a_fields_text_and_selection_changes_do_not_raise_structure_events() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "ab", 8.0);
    screen.publish(&mut uia);
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised);
    raised.clear();

    screen.retype(field, "abc");
    screen.reselect(field, 3, 3);
    screen.publish(&mut uia);

    uia.take_pending_for_test(&mut raised);
    let id = screen.control(field);
    assert_eq!(
        raised,
        vec![
            Raise::Property(id, Property::Text, Val::Text(utf16("ab").into())),
            Raise::text_changed(id),
            Raise::selection_changed(id),
        ]
    );
}

/// A password field publishes no text and no selection, so it owes neither event.
#[test]
fn a_password_field_raises_no_text_or_selection_event() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "ab", 8.0);
    screen.snapshot.fields[0].password = true;
    screen.publish(&mut uia);
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised);
    raised.clear();

    screen.retype(field, "abc");
    screen.reselect(field, 3, 3);
    screen.publish(&mut uia);

    uia.take_pending_for_test(&mut raised);
    assert!(raised.is_empty());
}

/// A control's number is what the publish states, not what the tree before it announced.
///
/// Without this a slider a client had just written would read back whatever the previous tree
/// happened to hold, which is a number and therefore indistinguishable from the truth.
#[test]
fn a_publish_states_where_a_control_stands() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let slider = screen.slider(NONE, (0.0, 0.0, 100.0, 24.0), Range::new(-24.0, 24.0));
    screen.snapshot.values.push((slider, 6.0));
    screen.publish(&mut uia);
    assert_eq!(uia.tree().value(slider), Some(6.0));

    // A republish that states a different number replaces it rather than carrying the old one.
    let mut next = screen.successor();
    let slider = next.slider(NONE, (0.0, 0.0, 100.0, 24.0), Range::new(-24.0, 24.0));
    next.snapshot.values.push((slider, -3.0));
    next.publish(&mut uia);
    assert_eq!(uia.tree().value(slider), Some(-3.0));
}

/// A pattern with nothing behind it is not advertised.
///
/// A graph that reports no number would answer `Minimum` with a failure and `Value` with its
/// own name, and a client cannot tell either from an answer.
#[test]
fn a_pattern_with_no_data_behind_it_is_not_advertised() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let graph = screen.add(NONE, (0.0, 0.0, 100.0, 24.0), UiaRole::Graph, "spectrum");
    let slider = screen.slider(NONE, (0.0, 24.0, 100.0, 48.0), Range::new(0.0, 1.0));
    screen.publish(&mut uia);

    let patterns = uia.tree().patterns(graph);
    assert!(
        !patterns.has(Patterns::RANGE),
        "a graph with no bounds offered RangeValue"
    );
    assert!(
        !patterns.has(Patterns::VALUE),
        "a graph with no number offered Value"
    );
    assert!(uia.tree().patterns(slider).has(Patterns::RANGE));
    assert!(uia.tree().patterns(slider).has(Patterns::VALUE));
}

/// Every element carries an automation id, and elements that share a name are still apart.
#[test]
fn every_element_is_addressable() {
    let mut screen = Screen::new();
    let card = screen.add(NONE, (0.0, 0.0, 100.0, 60.0), UiaRole::Group, "Gain");
    let inside = screen.add(card, (0.0, 0.0, 20.0, 20.0), UiaRole::Button, "Expand");
    let other = screen.add(NONE, (0.0, 60.0, 100.0, 120.0), UiaRole::Group, "Gain");
    let twin = screen.add(other, (0.0, 60.0, 20.0, 80.0), UiaRole::Button, "Expand");
    let mut seen = Vec::new();
    derive_keys(&mut screen.snapshot, &mut seen);
    let tree = Tree::adopt(&screen.snapshot, &[]);

    let key = |at: u16| String::from_utf16_lossy(tree.key(at));
    assert_eq!(key(card), "gain");
    assert_eq!(key(inside), "gain.expand");
    assert_eq!(
        key(other),
        "gain#2",
        "two groups with one name were not told apart"
    );
    assert_eq!(
        key(twin),
        "gain.expand#2",
        "a control's id must not depend on which of two identical cards it sits in being first"
    );
}

/// A scroll container reports how far its content can travel and how much of it is shown.
#[test]
fn a_container_reports_what_it_scrolls() {
    let mut screen = Screen::new();
    let viewport = screen.add(NONE, (0.0, 0.0, 100.0, 100.0), UiaRole::Group, "chain");
    screen.snapshot.entries[viewport as usize].flags =
        screen.snapshot.entries[viewport as usize].flags | ColFlags::SCROLLS;
    screen.snapshot.scrolls.push(ScrollView {
        node: NodeId::FIRST,
        owner: viewport,
        view: Vector2 { x: 100.0, y: 100.0 },
        content: Vector2 { x: 100.0, y: 400.0 },
    });
    let tree = Tree::adopt(&screen.snapshot, &[]);

    assert!(tree.patterns(viewport).has(Patterns::SCROLL));
    let (view, offset) = tree.viewport(viewport).expect("the container is a row");
    assert_eq!(view.travel().y, 300.0);
    assert_eq!(
        view.travel().x,
        0.0,
        "an axis with no overflow does not travel"
    );
    assert_eq!(offset.y, 0.0);

    tree.set_scroll(NodeId::FIRST, Vector2 { x: 0.0, y: 150.0 });
    assert_eq!(tree.viewport(viewport).expect("still a row").1.y, 150.0);
}

/// A container's reported position is the tracker's own word, not a copy taken at the publish.
///
/// The front thread does not tick while a flick settles, so a copy would leave every rectangle
/// inside the container at where the content was when the last contact ended.
#[test]
fn a_container_reports_where_its_content_is_now() {
    let mut screen = Screen::new();
    let viewport = screen.add(NONE, (0.0, 0.0, 100.0, 100.0), UiaRole::Group, "chain");
    screen.snapshot.scrolls.push(ScrollView {
        node: NodeId::FIRST,
        owner: viewport,
        view: Vector2 { x: 100.0, y: 100.0 },
        content: Vector2 { x: 100.0, y: 400.0 },
    });
    let shadow = Arc::new(AtomicU64::new(0));
    let tree = Tree::adopt(&screen.snapshot, &[(NodeId::FIRST, Arc::clone(&shadow))]);

    shadow.store(windows_scene::pack_offset(0.0, 90.0), Relaxed);
    assert_eq!(
        tree.viewport(viewport).expect("the container is a row").1.y,
        90.0,
        "the tree read a copy rather than the tracker's word"
    );
}

/// An overlay opening and closing is what a client is told, and a close names the root.
///
/// Derived from the published trees, so a menu dismissed by a press outside — which tells the
/// layer that opened it nothing — still reaches a client.
#[test]
fn an_overlay_opening_and_closing_is_raised() {
    let mut uia = listening();
    let mut screen = Screen::new();
    screen.add(NONE, (0.0, 0.0, 200.0, 40.0), UiaRole::Button, "Pick");
    screen.publish(&mut uia);
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised);

    // The menu arrives as a root of its own, which is the shape a slot root publishes in.
    let mut open = screen.successor();
    open.add(NONE, (0.0, 0.0, 200.0, 40.0), UiaRole::Button, "Pick");
    let menu = open.add(NONE, (0.0, 40.0, 200.0, 120.0), UiaRole::Menu, "");
    open.snapshot.entries[menu as usize].flags =
        open.snapshot.entries[menu as usize].flags | ColFlags::OVERLAY;
    let menu_id = open.control(menu);
    open.publish(&mut uia);
    raised.clear();
    uia.take_pending_for_test(&mut raised);
    assert!(
        raised.contains(&Raise::menu_opened(menu_id)),
        "a menu opened without saying so: {raised:?}"
    );

    let mut shut = open.successor();
    shut.add(NONE, (0.0, 0.0, 200.0, 40.0), UiaRole::Button, "Pick");
    shut.publish(&mut uia);
    raised.clear();
    uia.take_pending_for_test(&mut raised);
    assert!(
        raised.contains(&Raise::menu_closed(ControlId::NONE)),
        "a menu closed without saying so: {raised:?}"
    );
}

/// Missing subscription information does not suppress an event.
///
/// Two clients advising one event and one of them leaving must not silence it for the other,
/// which is why the table counts rather than holds a set.
#[test]
fn only_an_explicitly_unsubscribed_event_is_suppressed() {
    let advised = events::Advised::default();
    assert!(
        advised.wanted_for_test(1),
        "an empty table admitted nothing"
    );

    advised.added(1);
    assert!(advised.wanted_for_test(1));
    assert!(
        advised.wanted_for_test(2),
        "an unknown subscription was treated as absent"
    );

    advised.added(1);
    advised.removed(1);
    assert!(
        advised.wanted_for_test(1),
        "one client leaving silenced the other"
    );
    advised.removed(1);
    assert!(!advised.wanted_for_test(1));
    assert!(advised.wanted_for_test(2));
}

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn structural_events_distinguish_add_remove_reorder_and_ignore_property_changes() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let parent = screen.add(NONE, (0.0, 0.0, 100.0, 100.0), UiaRole::Group, "Group");
    let first = screen.add(parent, (0.0, 0.0, 100.0, 24.0), UiaRole::Button, "First");
    screen.publish(&mut uia);
    let mut events = Vec::new();
    uia.take_pending_for_test(&mut events);
    events.clear();
    let second = screen.add(parent, (0.0, 24.0, 100.0, 48.0), UiaRole::Button, "Second");
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    assert_eq!(
        events,
        vec![Raise::Structure(
            screen.control(parent),
            StructureChangeType_ChildrenBulkAdded
        )]
    );
    events.clear();
    screen
        .snapshot
        .entries
        .swap(first as usize, second as usize);
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    assert_eq!(
        events,
        vec![Raise::Structure(
            screen.control(parent),
            StructureChangeType_ChildrenReordered
        )]
    );
    events.clear();
    screen.snapshot.entries.pop();
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    assert_eq!(
        events,
        vec![Raise::Structure(
            screen.control(parent),
            StructureChangeType_ChildrenBulkRemoved
        )]
    );
    events.clear();
    screen.snapshot.entries[1].name = screen.snapshot.intern("Renamed");
    screen.snapshot.entries[1].box_[2] = 80.0;
    screen.snapshot.state[1] = State::default();
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    let id = screen.control(second);
    assert!(events.contains(&Raise::Property(
        id,
        Property::Native(UIA_NamePropertyId),
        Val::Text(utf16("Second").into())
    )));
    assert!(events.contains(&Raise::Property(
        id,
        Property::Native(UIA_IsEnabledPropertyId),
        Val::Bool(true)
    )));
    assert!(events.contains(&Raise::Property(
        id,
        Property::Native(UIA_BoundingRectanglePropertyId),
        Val::Rect([0.0, 24.0, 100.0, 24.0])
    )));
    assert!(!events.iter().any(|e| matches!(e, Raise::Structure(..))));
    events.clear();
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    assert!(events.is_empty());
}

#[test]
fn dialogs_queue_open_but_not_retired_close_or_menu_events() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let dialog = screen.add(NONE, (0.0, 0.0, 100.0, 100.0), UiaRole::Group, "Inspector");
    screen.snapshot.entries[dialog as usize].flags = ColFlags::OVERLAY | ColFlags::DIALOG;
    screen.publish(&mut uia);
    let mut events = Vec::new();
    uia.take_pending_for_test(&mut events);
    assert!(events.contains(&Raise::Event(
        screen.control(dialog),
        UIA_Window_WindowOpenedEventId
    )));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Raise::Event(_, UIA_MenuOpenedEventId)))
    );
    events.clear();
    screen.snapshot.entries.clear();
    screen.publish(&mut uia);
    uia.take_pending_for_test(&mut events);
    assert!(!events.contains(&Raise::Event(
        screen.control(dialog),
        UIA_Window_WindowClosedEventId
    )));
    assert!(uia.current.index_of(screen.control(dialog)).is_none());
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Raise::Event(_, UIA_MenuClosedEventId)))
    );
}

#[test]
fn tracker_notifications_report_scroll_percent_bounds_and_offscreen_without_structure() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 100.0, 50.0), UiaRole::Group, "Scroll");
    let child = screen.add(group, (0.0, 100.0, 100.0, 124.0), UiaRole::Button, "Below");
    let mut nodes = windows_scene::Ids::<{ windows_scene::NODE }>::default();
    let node = nodes.mint();
    screen.scrolls(group, node, &[child]);
    screen.snapshot.scrolls[0].view = Vector2 { x: 100.0, y: 50.0 };
    screen.snapshot.scrolls[0].content = Vector2 { x: 100.0, y: 150.0 };
    screen.publish(&mut uia);
    let mut events = Vec::new();
    uia.take_pending_for_test(&mut events);
    events.clear();
    uia.set_scroll(node, Vector2 { x: 0.0, y: 100.0 });
    uia.take_pending_for_test(&mut events);
    assert!(events.contains(&Raise::Property(
        screen.control(group),
        Property::Native(UIA_ScrollVerticalScrollPercentPropertyId),
        Val::Number(0.0)
    )));
    assert!(events.contains(&Raise::Property(
        screen.control(child),
        Property::Native(UIA_IsOffscreenPropertyId),
        Val::Bool(true)
    )));
    assert!(!events.iter().any(|e| matches!(e, Raise::Structure(..))));
    events.clear();
    uia.scroll_changed();
    uia.take_pending_for_test(&mut events);
    assert!(events.is_empty());
}

#[test]
fn presented_readings_notify_only_subscribers_and_coalesce_until_sync() {
    use std::sync::{Arc,atomic::AtomicU64};
    let mut uia = listening();
    let mut screen = Screen::new();
    let region = screen.add(NONE,(0.0,0.0,100.0,40.0),UiaRole::Graph,"measurements");
    screen.publish(&mut uia);
    let id = screen.control(region);
    let updates = Arc::new(PartUpdates::default());
    let values: Arc<[AtomicU64]> = vec![AtomicU64::new(MISSING_READING)].into();
    uia.watch_region(RegionPeer { format:None,id,geometry:Arc::new(windows_present::RegionParts::new()),parts:vec![PartDecl::new(0,"LUFS",UiaRole::Text).formatted(|value| value.map_or_else(|| "Unavailable".into(),|v| format!("{v:.1} LUFS")))],values:Some(values.clone()),value:None,updates:Some(updates.clone()) });
    let mut raised = Vec::new();
    uia.take_pending_for_test(&mut raised); raised.clear();
    values[0].store((-14.0f64).to_bits(),Relaxed);
    updates.changed(1);
    uia.sync_regions();
    uia.take_pending_for_test(&mut raised);
    assert!(raised.is_empty());
    uia.shared.advised.added(crate::bindings::UIA_AutomationPropertyChangedEventId);
    for value in [-13.0f64,-12.0,-11.0] {
        values[0].store(value.to_bits(),Relaxed); updates.changed(1);
    }
    uia.sync_regions();
    uia.take_pending_for_test(&mut raised);
    assert_eq!(raised.iter().filter(|event| matches!(event,Raise::PartValue(..))).count(),1);
    raised.clear();
    updates.changed(1); uia.sync_regions(); uia.take_pending_for_test(&mut raised);
    assert!(raised.is_empty());
    uia.shared.regions.forget(id);
    assert!(updates.shared.lock().unwrap().upgrade().is_none());
    updates.changed(1); uia.sync_regions(); uia.take_pending_for_test(&mut raised);
    assert!(raised.is_empty());
}

#[test]
fn translation_targets_update_existing_providers_and_announce_geometry_once() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 10.0, 100.0, 60.0), UiaRole::Group, "card");
    let child = screen.add(group, (10.0, 20.0, 80.0, 50.0), UiaRole::Button, "preview");
    let state = windows_scene::Translation::new(Vector2::new(0.0, -3.0));
    screen.snapshot.translations.push(windows_scene::TranslationRange {
        owner: screen.control(group), start: 0, end: 2, state: state.clone(),
    });
    screen.publish(&mut uia);
    let held = uia.current.clone();
    let before = held.bounds(child);
    let mut events = Vec::new();
    uia.take_pending_for_test(&mut events); events.clear();
    state.set_active(true);
    assert_eq!(held.shifted(child), [10.0, 17.0, 80.0, 47.0]);
    assert_eq!(held.hit(Point::new(20.0, 18.0)), Some(child));
    uia.translation_changed();
    uia.take_pending_for_test(&mut events);
    assert!(events.contains(&Raise::Property(screen.control(child),
        Property::Native(UIA_BoundingRectanglePropertyId), Val::Rect(before))));
    assert!(!events.iter().any(|e| matches!(e, Raise::Structure(..))));
    events.clear();
    uia.translation_changed();
    uia.take_pending_for_test(&mut events);
    assert!(events.is_empty());
    state.set_active(false);
    assert_eq!(held.bounds(child), before);
}
