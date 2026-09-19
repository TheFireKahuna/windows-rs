//! Tests for reconciliation, asserting the steps emitted rather than the final order.
//!
//! Any correct reconciler ends in the right order; these tests check that this one reaches
//! it with the minimum number of moves and without rebuilding a survivor, which only the
//! step list shows.

use super::*;
use crate::signal::{Cell, Owner, live_nodes};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static REMOVED: RefCell<Vec<char>> = const { RefCell::new(Vec::new()) };
}

struct Retained(char, Cell<i32>);
impl Drop for Retained {
    fn drop(&mut self) {
        assert!(self.1.alive(), "result must drop before its signal scope");
        REMOVED.with(|out| out.borrow_mut().push(self.0));
    }
}

/// Runs a reconcile and records what each key was told to do.
#[derive(Default)]
struct Recorder {
    removed: Vec<char>,
    built: Vec<char>,
    steps: Vec<(char, Step)>,
}

impl Recorder {
    fn run(list: &mut Keyed<char, Retained>, next: &str) -> Self {
        // The item is its own key here, so the projection is the identity: no pair to
        // build, and nothing stored twice.
        let items: Vec<char> = next.chars().collect();
        let out = Rc::new(RefCell::new(Self::default()));
        let (built, placed) = (Rc::clone(&out), Rc::clone(&out));
        REMOVED.with(|out| out.borrow_mut().clear());
        list.reconcile(
            &items,
            |item| item,
            move |item| {
                built.borrow_mut().built.push(*item);
                Retained(*item, Cell::new(0))
            },
            move |row, step| placed.borrow_mut().steps.push((row.0, step)),
        );
        out.borrow_mut().removed = REMOVED.with(|log| core::mem::take(&mut *log.borrow_mut()));
        Rc::try_unwrap(out)
            .unwrap_or_else(|_| unreachable!("the callbacks are dropped by now"))
            .into_inner()
    }

    fn keys(&self) -> Vec<char> {
        self.steps.iter().map(|(key, _)| *key).collect()
    }

    fn of(&self, want: Step) -> Vec<char> {
        self.steps
            .iter()
            .filter(|(_, step)| *step == want)
            .map(|(key, _)| *key)
            .collect()
    }

    fn moved(&self) -> Vec<char> {
        self.of(Step::Move)
    }

    fn kept(&self) -> Vec<char> {
        self.of(Step::Keep)
    }

    fn inserted(&self) -> Vec<char> {
        self.of(Step::Insert)
    }
}

#[test]
fn a_longest_increasing_subsequence_is_the_move_set_complement() {
    assert_eq!(compute_lis(&[]), Vec::<usize>::new());
    assert_eq!(compute_lis(&[5]), vec![0]);
    assert_eq!(compute_lis(&[0, 1, 2, 3]), vec![0, 1, 2, 3]);
    // Strictly decreasing: everything but one element has to move.
    assert_eq!(compute_lis(&[3, 2, 1, 0]).len(), 1);
    // The textbook case. 2, 3, 7, 101 is one of the length-4 answers.
    let seq = [10, 9, 2, 5, 3, 7, 101, 18];
    let lis = compute_lis(&seq);
    assert_eq!(lis.len(), 4);
    assert!(
        lis.windows(2).all(|w| seq[w[0]] < seq[w[1]]),
        "the result must be increasing in the input"
    );
    assert!(
        lis.windows(2).all(|w| w[0] < w[1]),
        "and in its own indices"
    );
}

#[test]
fn an_unchanged_list_moves_nothing_and_rebuilds_nothing() {
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abcd");
    let out = Recorder::run(&mut list, "abcd");
    assert!(out.built.is_empty());
    assert!(out.removed.is_empty());
    assert_eq!(out.kept(), vec!['a', 'b', 'c', 'd']);
}

#[test]
fn a_row_added_at_the_head_moves_no_survivor() {
    // A reconciler without a subsequence moves every row after the insertion, which for a
    // list that gained one row at the top is the whole list.
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abc");
    let out = Recorder::run(&mut list, "zabc");
    assert_eq!(out.inserted(), vec!['z']);
    assert!(out.moved().is_empty(), "moved {:?}", out.moved());
    assert_eq!(out.kept(), vec!['a', 'b', 'c']);
}

#[test]
fn a_reorder_moves_the_minimum() {
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abcde");
    // One row taken from the end and put at the front: one move, not five.
    let out = Recorder::run(&mut list, "eabcd");
    assert_eq!(out.moved(), vec!['e']);
    assert_eq!(out.kept(), vec!['a', 'b', 'c', 'd']);
    assert_eq!(out.keys(), vec!['e', 'a', 'b', 'c', 'd']);
}

#[test]
fn a_reversal_moves_all_but_one() {
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abcd");
    let out = Recorder::run(&mut list, "dcba");
    assert_eq!(out.moved().len(), 3);
    assert_eq!(out.kept().len(), 1);
}

#[test]
fn a_departing_row_is_told_before_anything_is_built() {
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abc");
    let out = Recorder::run(&mut list, "axc");
    assert_eq!(out.removed, vec!['b']);
    assert_eq!(out.built, vec!['x']);
    assert_eq!(out.keys(), vec!['a', 'x', 'c']);
}

#[test]
fn a_departing_row_disposes_its_scope_and_a_surviving_one_does_not() {
    let baseline = live_nodes();
    let mut list: Keyed<char> = Keyed::new();
    let cells: Rc<RefCell<Vec<(char, Cell<i32>)>>> = Rc::new(RefCell::new(Vec::new()));

    let build = |cells: &Rc<RefCell<Vec<(char, Cell<i32>)>>>| {
        let cells = Rc::clone(cells);
        move |item: &char| {
            // Created inside the row's own scope, so the row owns it.
            cells.borrow_mut().push((*item, Cell::new(0_i32)));
        }
    };

    let items: Vec<char> = "abc".chars().collect();
    list.reconcile(&items, |item| item, build(&cells), |_, _| {});
    assert_eq!(live_nodes(), baseline + 3);

    let items: Vec<char> = "ac".chars().collect();
    list.reconcile(&items, |item| item, build(&cells), |_, _| {});
    assert_eq!(
        live_nodes(),
        baseline + 2,
        "the departing row's cell survived"
    );

    let live: Vec<char> = cells
        .borrow()
        .iter()
        .filter(|(_, cell)| cell.alive())
        .map(|(key, _)| *key)
        .collect();
    assert_eq!(live, vec!['a', 'c']);

    drop(list);
    assert_eq!(live_nodes(), baseline);
}

#[test]
fn a_list_reconciled_from_inside_an_effect_does_not_grow_its_scope() {
    // A row's scope is detached, so reconciling from an effect does not register every row
    // it ever built as a child of the effect's own scope, which would grow for the life of
    // the screen.
    let baseline = live_nodes();
    let (owner, ()) = Owner::scope(|| {
        let mut list: Keyed<u32> = Keyed::new();
        for round in 0..100_u32 {
            let items: Vec<u32> = (round..round + 3).collect();
            list.reconcile(
                &items,
                |item| item,
                |_| {
                    let _ = Cell::new(0_i32);
                },
                |_, _| {},
            );
        }
        assert_eq!(
            live_nodes(),
            baseline + 3,
            "only the live rows' cells remain"
        );
    });
    drop(owner);
    assert_eq!(live_nodes(), baseline);
}

#[test]
fn a_branch_builds_once_per_key_change_and_never_for_a_repeat() {
    let baseline = live_nodes();
    let (owner, ()) = Owner::scope(|| {
        let log = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut branch: Branch<&'static str, Teardown> = Branch::new();

        let build = |log: &Rc<RefCell<Vec<String>>>| {
            let log = Rc::clone(log);
            move |key: &&'static str| {
                log.borrow_mut().push(format!("build {key}"));
                Teardown {
                    key: *key,
                    cell: Cell::new(0_i32),
                    log: log.clone(),
                }
            }
        };

        branch.set(Some("home"), build(&log));
        branch.set(Some("home"), build(&log));
        assert_eq!(*log.borrow(), ["build home"], "a repeat is not a change");
        assert_eq!(live_nodes(), baseline + 1);

        branch.set(Some("effects"), build(&log));
        assert_eq!(
            *log.borrow(),
            ["build home", "teardown home", "build effects"],
            "the outgoing arm is told while its nodes still exist"
        );
        assert_eq!(
            live_nodes(),
            baseline + 1,
            "the old arm's cell went with it"
        );

        branch.close();
        assert!(!branch.is_open());
        // Absence contributes nothing: no node, no placeholder.
        assert_eq!(live_nodes(), baseline);
    });
    drop(owner);
    assert_eq!(live_nodes(), baseline);
}

struct Teardown {
    key: &'static str,
    cell: Cell<i32>,
    log: Rc<RefCell<Vec<String>>>,
}
impl Drop for Teardown {
    fn drop(&mut self) {
        assert!(self.cell.alive());
        self.log.borrow_mut().push(format!("teardown {}", self.key));
    }
}

#[test]
fn a_keyed_reconcile_allocates_nothing_after_the_first() {
    // Two rounds warm both halves of the scratch: the first builds every row, and the second
    // is the first with a survivor in every seat, which is what grows the subsequence buffers.
    let rows: Vec<u32> = (0..64).collect();
    let mut list: Keyed<u32> = Keyed::new();
    list.reconcile(&rows, |item| item, |_| {}, |_, _| {});
    list.reconcile(&rows, |item| item, |_| {}, |_, _| {});

    let rotated: Vec<u32> = rows.iter().rev().copied().collect();
    list.reconcile(&rotated, |item| item, |_| {}, |_, _| {});

    let before = crate::counting::allocations();
    for order in [&rows, &rotated] {
        list.reconcile(order, |item| item, |_| {}, |_, _| {});
    }
    assert_eq!(
        crate::counting::allocations() - before,
        0,
        "a reconcile of a settled key set allocated"
    );
}
