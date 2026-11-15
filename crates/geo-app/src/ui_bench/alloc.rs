//! A global allocator that counts the allocations of one thread while
//! counting is switched on. Installed only in the test binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Passes every call to the system allocator and, on a thread that has
/// called [`start`], counts allocations and reallocations with their sizes.
pub struct Counting;

fn note(size: usize) {
    // `try_with`: the thread-local storage may be gone while a thread exits.
    if ON.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        let _ = BYTES.try_with(|b| b.set(b.get() + size as u64));
    }
}

// SAFETY: every method forwards to `System` with the same arguments; the
// counters are plain thread-local cells without destructors, so touching
// them cannot allocate or recurse.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Resets the counters of this thread and starts counting.
pub fn start() {
    COUNT.with(|c| c.set(0));
    BYTES.with(|b| b.set(0));
    ON.with(|o| o.set(true));
}

/// Stops counting and returns `(allocations, bytes)` since [`start`].
pub fn stop() -> (u64, u64) {
    ON.with(|o| o.set(false));
    (COUNT.with(Cell::get), BYTES.with(Cell::get))
}
