//! Counts heap allocations per thread, so a zero-allocation path can be measured.
//!
//! A structural check — a buffer that did not grow, a pending stack that came back empty —
//! cannot see a temporary allocated and freed inside the call it measures. A count can.
//!
//! The counter is per thread. Tests run in parallel, so a process-wide counter would attribute
//! another test's allocations to the one under measurement; here every allocation counts
//! against the thread that made it.
//!
//! Test-only. Nothing here is compiled into the library.

use core::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};

thread_local! {
    /// Counts this thread's allocations. `const` init, so the first access neither allocates
    /// nor registers a destructor; either would re-enter the allocator.
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

/// Returns the number of allocations this thread has made.
///
/// Read it either side of the statement under test and take the difference; the harness
/// allocates between tests, so the absolute value carries nothing. Returns 0 once the thread's
/// locals have been destroyed.
#[must_use]
pub fn allocations() -> usize {
    // A fallible access: this can be reached while the thread's locals are being destroyed,
    // and a panic inside the allocator has nowhere to go.
    COUNT.try_with(Cell::get).unwrap_or(0)
}

fn count() {
    COUNT.try_with(|n| n.set(n.get() + 1)).ok();
}

/// Counts an allocation, then delegates to `System`, whose contract it leaves unchanged.
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

    /// Counts a growth. A buffer already at its high-water mark never reaches this.
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
