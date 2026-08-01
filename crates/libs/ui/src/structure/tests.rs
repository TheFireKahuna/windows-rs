//! What reconciliation claims, asserted as the **steps it emitted** rather than as the
//! order it ended up in.
//!
//! Any correct reconciler ends up in the right order. The whole value of this one is that
//! it gets there with the minimum number of moves and without rebuilding a survivor, and
//! only the step list shows that.

use super::*;
use crate::signal::{Cell, Owner, live_nodes};
use std::cell::RefCell;
use std::rc::Rc;

/// Runs a reconcile and records what each key was told to do.
#[derive(Default)]
struct Recorder {
    removed: Vec<char>,
    built: Vec<char>,
    steps: Vec<(char, Step, Option<usize>)>,
}

impl Recorder {
    fn run(list: &mut Keyed<char>, next: &str) -> Self {
        let items: Vec<(char, ())> = next.chars().map(|key| (key, ())).collect();
        let out = Rc::new(RefCell::new(Self::default()));
        let (removed, built, placed) = (Rc::clone(&out), Rc::clone(&out), Rc::clone(&out));
        list.reconcile(
            &items,
            move |key| removed.borrow_mut().removed.push(*key),
            move |key, ()| built.borrow_mut().built.push(*key),
            move |key, (), step, after| placed.borrow_mut().steps.push((*key, step, after)),
        );
        Rc::try_unwrap(out)
            .unwrap_or_else(|_| unreachable!("the callbacks are dropped by now"))
            .into_inner()
    }

    fn moved(&self) -> Vec<char> {
        self.steps
            .iter()
            .filter(|(_, step, _)| *step == Step::Move)
            .map(|(key, _, _)| *key)
            .collect()
    }

    fn kept(&self) -> Vec<char> {
        self.steps
            .iter()
            .filter(|(_, step, _)| *step == Step::Keep)
            .map(|(key, _, _)| *key)
            .collect()
    }

    fn inserted(&self) -> Vec<char> {
        self.steps
            .iter()
            .filter(|(_, step, _)| *step == Step::Insert)
            .map(|(key, _, _)| *key)
            .collect()
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
    // The case a reconciler without a subsequence gets wrong: it moves every row after the
    // insertion, which for a list that gained one row at the top is the whole list.
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
    assert_eq!(list.keys(), ['e', 'a', 'b', 'c', 'd']);
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
fn a_survivor_is_placed_after_the_key_already_in_front_of_it() {
    // Front to back, so the predecessor is already correct when a step is applied — which
    // is what lets the caller use "insert after this one" and nothing else.
    let mut list = Keyed::new();
    let out = Recorder::run(&mut list, "abc");
    let afters: Vec<Option<usize>> = out.steps.iter().map(|(_, _, after)| *after).collect();
    assert_eq!(afters, vec![None, Some(0), Some(1)]);
}

#[test]
fn a_departing_row_is_told_before_anything_is_built() {
    let mut list = Keyed::new();
    Recorder::run(&mut list, "abc");
    let out = Recorder::run(&mut list, "axc");
    assert_eq!(out.removed, vec!['b']);
    assert_eq!(out.built, vec!['x']);
    assert_eq!(list.len(), 3);
}

#[test]
fn a_departing_row_disposes_its_scope_and_a_surviving_one_does_not() {
    let baseline = live_nodes();
    let mut list: Keyed<char> = Keyed::new();
    let cells: Rc<RefCell<Vec<(char, Cell<i32>)>>> = Rc::new(RefCell::new(Vec::new()));

    let build = |cells: &Rc<RefCell<Vec<(char, Cell<i32>)>>>| {
        let cells = Rc::clone(cells);
        move |key: &char, _: &()| {
            // Created inside the row's own scope, so the row owns it.
            cells.borrow_mut().push((*key, Cell::new(0_i32)));
        }
    };

    let items: Vec<(char, ())> = "abc".chars().map(|k| (k, ())).collect();
    list.reconcile(&items, |_| {}, build(&cells), |_, _, _, _| {});
    assert_eq!(live_nodes(), baseline + 3);

    let items: Vec<(char, ())> = "ac".chars().map(|k| (k, ())).collect();
    list.reconcile(&items, |_| {}, build(&cells), |_, _, _, _| {});
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

    list.clear(|_| {});
    assert_eq!(live_nodes(), baseline);
}

#[test]
fn a_list_reconciled_from_inside_an_effect_does_not_grow_its_scope() {
    // A row's scope is detached, so reconciling from an effect does not register every row
    // it ever built as a child of the effect's own scope — which would grow for the life of
    // the screen and is invisible until a profile shows the arena climbing.
    let baseline = live_nodes();
    let (owner, ()) = Owner::scope(|| {
        let mut list: Keyed<u32> = Keyed::new();
        for round in 0..100_u32 {
            let items: Vec<(u32, ())> = (round..round + 3).map(|key| (key, ())).collect();
            list.reconcile(
                &items,
                |_| {},
                |_, ()| {
                    let _ = Cell::new(0_i32);
                },
                |_, _, _, _| {},
            );
        }
        assert_eq!(
            live_nodes(),
            baseline + 3,
            "only the live rows' cells remain"
        );
        list.clear(|_| {});
    });
    drop(owner);
    assert_eq!(live_nodes(), baseline);
}

#[test]
fn a_branch_builds_once_per_key_change_and_never_for_a_repeat() {
    let baseline = live_nodes();
    let (owner, ()) = Owner::scope(|| {
        let log = Rc::new(RefCell::new(Vec::<String>::new()));
        let mut branch: Branch<&'static str> = Branch::new();

        let build = |log: &Rc<RefCell<Vec<String>>>| {
            let log = Rc::clone(log);
            move |key: &&'static str| {
                log.borrow_mut().push(format!("build {key}"));
                let _ = Cell::new(0_i32);
            }
        };
        let teardown = |log: &Rc<RefCell<Vec<String>>>| {
            let log = Rc::clone(log);
            move |key: &&'static str| log.borrow_mut().push(format!("teardown {key}"))
        };

        branch.set(Some("home"), teardown(&log), build(&log));
        branch.set(Some("home"), teardown(&log), build(&log));
        assert_eq!(*log.borrow(), ["build home"], "a repeat is not a change");
        assert_eq!(live_nodes(), baseline + 1);

        branch.set(Some("effects"), teardown(&log), build(&log));
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

        branch.close(teardown(&log));
        assert!(!branch.is_open());
        // Absence contributes nothing: no node, no placeholder.
        assert_eq!(live_nodes(), baseline);
    });
    drop(owner);
    assert_eq!(live_nodes(), baseline);
}
