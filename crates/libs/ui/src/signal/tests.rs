//! What the scheme claims, asserted as **evaluation counts** rather than as values.
//!
//! Every failure this module exists to catch — a glitch, a lost cutoff, a leaked
//! subscription, a wasted recompute — produces the right answer at the wrong cost or on
//! the wrong pass. Asserting on the value would miss all four.

use super::*;
use std::cell::RefCell as Slot;
use std::rc::Rc as Ref;

/// Records what ran, in order.
#[derive(Default)]
struct Log(Slot<Vec<&'static str>>);

impl Log {
    fn push(&self, what: &'static str) {
        self.0.borrow_mut().push(what);
    }

    fn count(&self, what: &str) -> usize {
        self.0
            .borrow()
            .iter()
            .filter(|entry| **entry == what)
            .count()
    }

    fn take(&self) -> Vec<&'static str> {
        core::mem::take(&mut self.0.borrow_mut())
    }
}

#[test]
fn a_write_that_changes_nothing_propagates_nothing() {
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(1_i32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = a.get();
            seen.push("effect");
        });
        assert_eq!(log.count("effect"), 1, "an effect runs once when created");

        a.set(1);
        flush();
        assert_eq!(log.count("effect"), 1, "an equal write is not a write");

        a.set(2);
        flush();
        assert_eq!(log.count("effect"), 2);
    });
}

#[test]
fn a_diamond_evaluates_its_shared_node_once() {
    // a → b, a → c, (b, c) → d. A naive depth-first push evaluates `d` twice and shows it
    // one old input in between, which is the glitch two-level marking exists to prevent.
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(1_i32);
        let log = Ref::new(Log::default());

        let seen = Ref::clone(&log);
        let b = Memo::new(move || {
            seen.push("b");
            a.get() * 2
        });
        let seen = Ref::clone(&log);
        let c = Memo::new(move || {
            seen.push("c");
            a.get() * 3
        });
        let seen = Ref::clone(&log);
        let d = Memo::new(move || {
            seen.push("d");
            b.get() + c.get()
        });

        let seen = Ref::clone(&log);
        Effect::new(move || {
            seen.push("effect");
            let _ = d.get();
        });
        log.take();

        a.set(2);
        flush();
        assert_eq!(log.count("d"), 1, "the shared node ran more than once");
        assert_eq!(log.count("effect"), 1);
        assert_eq!(d.get(), 10);
    });
}

#[test]
fn a_diamond_whose_branches_disagree_does_not_answer_from_a_stale_cache() {
    // The shape that makes marking a settled derivation's subscribers `Clean` unsound: one
    // branch moves and the other does not, and whichever resolves second must not undo the
    // first's mark.
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(1_i32);
        // Moves with `a`.
        let moves = Memo::new(move || a.get() * 2);
        // Settles: every positive `a` clamps to the same 10.
        let settles = Memo::new(move || a.get().clamp(10, 10));
        let sum = Memo::new(move || moves.get() + settles.get());
        assert_eq!(sum.get(), 12);

        a.set(5);
        flush();
        assert_eq!(
            sum.get(),
            20,
            "the settled branch cleared the moved branch's mark"
        );
    });
}

#[test]
fn a_derivation_that_settles_stops_the_propagation() {
    // The cutoff. Without it, every write to `raw` wakes the effect even where the clamp
    // already held — the difference between an interaction costing one comparison and one
    // costing a subtree.
    let (_owner, ()) = Owner::scope(|| {
        let raw = Cell::new(5.0_f64);
        let clamped = Memo::new(move || raw.get().clamp(0.0, 1.0));
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = clamped.get();
            seen.push("effect");
        });
        assert_eq!(log.count("effect"), 1);

        raw.set(9.0);
        flush();
        assert_eq!(log.count("effect"), 1, "still clamped to the same 1.0");

        raw.set(0.5);
        flush();
        assert_eq!(log.count("effect"), 2);
    });
}

#[test]
fn a_memo_nothing_reads_never_runs() {
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(1_i32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        let memo = Memo::new(move || {
            seen.push("memo");
            a.get()
        });
        a.set(2);
        flush();
        assert_eq!(
            log.count("memo"),
            0,
            "laziness is the contract, not an optimization"
        );
        assert_eq!(memo.get(), 2);
        assert_eq!(log.count("memo"), 1);
    });
}

#[test]
fn effects_run_in_creation_order_after_every_memo() {
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(0_i32);
        let log = Ref::new(Log::default());

        let seen = Ref::clone(&log);
        let memo = Memo::new(move || {
            seen.push("memo");
            a.get() + 1
        });
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = memo.get();
            seen.push("first");
        });
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = memo.get();
            seen.push("second");
        });
        log.take();

        a.set(1);
        flush();
        assert_eq!(log.take(), vec!["memo", "first", "second"]);
    });
}

#[test]
fn a_branch_no_longer_read_stops_costing() {
    // The dependency set is rebuilt per run, so a hidden arm's inputs stop waking it.
    // Without that, a subtree behind a `false` keeps the cost it had when it was visible.
    let (_owner, ()) = Owner::scope(|| {
        let show = Cell::new(true);
        let hidden = Cell::new(0_i32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            if show.get() {
                let _ = hidden.get();
            }
            seen.push("effect");
        });
        assert_eq!(log.count("effect"), 1);

        hidden.set(1);
        flush();
        assert_eq!(log.count("effect"), 2, "read while shown");

        show.set(false);
        flush();
        assert_eq!(log.count("effect"), 3);

        hidden.set(2);
        flush();
        assert_eq!(log.count("effect"), 3, "the edge was dropped with the read");
    });
}

#[test]
fn a_thousand_mounts_leak_no_node() {
    let baseline = live_nodes();
    for _ in 0..1_000 {
        let (owner, ()) = Owner::scope(|| {
            let a = Cell::new(1_i32);
            let b = Memo::new(move || a.get() * 2);
            Effect::new(move || {
                let _ = b.get();
            });
            // A scope opened inside another is disposed by it, with no walk of its own —
            // which is the shape that leaked once per unmount in the stack this replaces.
            let (inner, ()) = Owner::scope(|| {
                let c = Cell::new(2_i32);
                Effect::new(move || {
                    let _ = c.get();
                });
            });
            core::mem::forget(inner);
        });
        drop(owner);
    }
    assert_eq!(live_nodes(), baseline);
}

#[test]
fn a_disposed_cell_answers_that_it_is_gone_rather_than_reading_a_stranger() {
    let stale = {
        let (owner, cell) = Owner::scope(|| Cell::new(7_i32));
        drop(owner);
        cell
    };
    assert!(!stale.alive());
    // The slot is recycled here, so the generation check is the only thing between the
    // stale handle and its new occupant.
    let (_owner, fresh) = Owner::scope(|| Cell::new(9_i32));
    assert_eq!(fresh.get(), 9);
    assert!(!stale.alive());
}

#[test]
fn an_effect_may_write_and_the_flush_settles() {
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(0_i32);
        let mirror = Cell::new(0_i32);
        Effect::new(move || mirror.set(a.get() * 10));
        assert_eq!(mirror.peek(), 0);

        a.set(3);
        flush();
        assert_eq!(mirror.peek(), 30, "the write re-entered and settled");
    });
}

#[test]
fn a_version_moves_only_when_the_value_does() {
    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(1_i32);
        let at = a.version();
        a.set(1);
        assert_eq!(a.version(), at);
        a.set(2);
        assert_eq!(a.version(), at + 1);
    });
}

#[test]
fn a_producer_thread_stages_a_write_and_the_flush_applies_it() {
    let (_owner, ()) = Owner::scope(|| {
        let level = Cell::new(0_u32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = level.get();
            seen.push("effect");
        });
        log.take();

        std::thread::spawn(move || {
            // A burst. The last value is what lands, and one wake is what is signalled.
            for value in 1..=100 {
                level.post(value);
            }
        })
        .join()
        .expect("the producer finished");

        flush();
        assert_eq!(level.peek(), 100, "writes coalesce to the last one");
        assert_eq!(log.count("effect"), 1, "and wake the graph once");
    });
}

#[test]
fn a_staged_write_against_a_disposed_cell_is_dropped() {
    let stale = {
        let (owner, cell) = Owner::scope(|| Cell::new(0_u32));
        drop(owner);
        cell
    };
    std::thread::spawn(move || stale.post(42))
        .join()
        .expect("the producer finished");
    let (_owner, fresh) = Owner::scope(|| Cell::new(7_u32));
    flush();
    assert_eq!(fresh.peek(), 7, "the write reached the slot's new occupant");
}

#[test]
fn a_constant_is_distinguishable_from_a_signal_without_reading_it() {
    // What lets a static label cost one sprite and no graph node. Most of a screen is
    // static, so this is not a micro-optimization.
    fn constant<M>(value: impl Signal<f32, M>) -> bool {
        value.is_constant()
    }
    let (_owner, ()) = Owner::scope(|| {
        let cell = Cell::new(1.0_f32);
        assert!(constant(0.6_f32));
        assert!(!constant(cell));
        assert!(!constant(move || cell.get()));
    });
}

#[test]
fn an_epoch_carries_a_count_a_consumer_can_miss_and_still_detect() {
    let epoch = Epoch::new().expect("an event is available");
    let at = epoch.count();
    for _ in 0..5 {
        epoch.bump();
    }
    assert_eq!(epoch.count(), at + 5);
    // Already signalled, so the wait returns without parking.
    assert_eq!(epoch.wait(0), at + 5);
}

/// A host that blocks needs telling that a write happened, and telling **once**.
///
/// Both halves matter and they fail in opposite directions: a wake that never comes is a
/// frozen window, and a wake per write is the spin the blocking pump exists to remove.
#[test]
fn a_write_asks_for_one_frame_and_a_burst_still_asks_once() {
    let asked = Ref::new(Slot::new(0_u32));
    let counter = Ref::clone(&asked);
    set_waker(move || *counter.borrow_mut() += 1);
    let asks = || *asked.borrow();

    let (_owner, ()) = Owner::scope(|| {
        let a = Cell::new(0_i32);
        let b = Cell::new(0_i32);
        let unread = Cell::new(0_i32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            let _ = a.get();
            let _ = b.get();
            seen.push("effect");
        });
        // The mount's own first run is the host's business, not a wake's: it flushes before
        // it shows the window. Whatever that cost, the counting starts after it.
        flush();
        let mounted = asks();

        a.set(1);
        assert_eq!(
            asks(),
            mounted + 1,
            "a write with a subscriber asks for a frame"
        );

        a.set(2);
        b.set(3);
        assert_eq!(
            asks(),
            mounted + 1,
            "the queue was already dirty, so these are the same frame"
        );

        flush();
        assert_eq!(log.count("effect"), 2, "and one frame ran the effect once");

        a.set(4);
        assert_eq!(asks(), mounted + 2, "a drained queue arms the edge again");

        flush();
        let before = asks();
        unread.set(9);
        assert_eq!(
            asks(),
            before,
            "nothing reads it, so there is nothing to draw and nothing to ask for"
        );
    });
}

/// A write from inside an effect asks for nothing: the flush it is already inside picks it
/// up on its next pass, and asking there would mean a frame after every frame.
#[test]
fn a_write_from_inside_a_flush_asks_for_no_further_frame() {
    let asked = Ref::new(Slot::new(0_u32));
    let counter = Ref::clone(&asked);
    set_waker(move || *counter.borrow_mut() += 1);

    let (_owner, ()) = Owner::scope(|| {
        let input = Cell::new(0_i32);
        let derived = Cell::new(0_i32);
        let log = Ref::new(Log::default());
        let seen = Ref::clone(&log);
        Effect::new(move || {
            derived.set(input.get() * 2);
            seen.push("writer");
        });
        // A second effect, so the write above genuinely has a subscriber to queue.
        let watched = Ref::new(Log::default());
        let watcher = Ref::clone(&watched);
        Effect::new(move || {
            let _ = derived.get();
            watcher.push("reader");
        });
        flush();

        let before = *asked.borrow();
        input.set(21);
        assert_eq!(*asked.borrow(), before + 1, "the outside write asks once");
        flush();
        assert_eq!(
            *asked.borrow(),
            before + 1,
            "and the effect's own write inside that flush asks for nothing"
        );
        assert_eq!(derived.get(), 42, "while still having happened");
        assert_eq!(watched.count("reader"), 2, "and still having propagated");
    });
}
