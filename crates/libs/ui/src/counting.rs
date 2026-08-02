//! A counting allocator, so "this path allocates nothing" can be measured rather than argued.
//!
//! Every other claim the lowering makes about allocation is structural — a buffer that did not
//! grow, a pending stack that came back empty — and all of them held while the mount walk was
//! allocating three `Vec`s per node. A temporary is invisible to a capacity check: it is
//! allocated and freed inside the call. Only a count sees it.
//!
//! **Per thread**, and that is what makes it usable: tests run in parallel, so a process-wide
//! counter would attribute whatever else was running to whatever is being measured. An
//! allocation is attributed to the thread that made it, which is exactly the question a
//! zero-allocation claim asks.
//!
//! Test-only. Nothing here is compiled into the library.

use core::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};

thread_local! {
    /// `const` init, so reaching this never allocates and registers no destructor — either
    /// would be the allocator calling itself.
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

/// How many times this thread has allocated.
///
/// Read it either side of the statement under test: the claim is a **difference**, because
/// the harness itself allocates between tests.
#[must_use]
pub fn allocations() -> usize {
    // Fallibly: this can be reached while the thread's locals are being destroyed, and a
    // panic inside the allocator has nowhere to go.
    COUNT.try_with(Cell::get).unwrap_or(0)
}

fn count() {
    COUNT.try_with(|n| n.set(n.get() + 1)).ok();
}

/// Counts, then delegates. The allocator contract is `System`'s, unchanged.
struct Counting;

// SAFETY: every method forwards to `System` with the arguments it was given and returns what
// `System` returned, so every guarantee is `System`'s. The only added effect is an increment
// on a thread-local `Cell<usize>`, which allocates nothing and cannot unwind.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    /// A growth counts. A buffer at high-water mark is precisely one that does not do this.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;
