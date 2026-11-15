//! Recomputing expression objects performs no heap allocations: neither
//! when their inputs carry no exact decimal (point coordinates, dependent
//! numbers, numbers that were set) nor in the common case where they carry
//! small ones. A counting global allocator, installed only in this test
//! binary, counts the allocations of the test's own thread.

use geo_kernel::script::load;
use geo_kernel::{Construction, Id, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

fn note() {
    // `try_with`: the thread-local storage may be gone while a thread exits.
    if ON.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

// SAFETY: every method forwards to `System` with the same arguments; the
// counters are plain thread-local cells without destructors, so touching
// them cannot allocate or recurse.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> u64 {
    COUNT.with(|c| c.set(0));
    ON.with(|o| o.set(true));
    f();
    ON.with(|o| o.set(false));
    COUNT.with(Cell::get)
}

fn id(c: &Construction, label: &str) -> Id {
    c.id(label).unwrap()
}

/// Moves `A` back and forth 1000 times; every move must recompute `watch`
/// and change its value. Returns the allocations of the 1000 moves.
fn drag(c: &mut Construction, watch: &str) -> u64 {
    let (a, w) = (id(c, "A"), id(c, watch));
    // warm up: the first move may grow buffers that are then reused
    c.move_point(a, 5.0, 6.0).unwrap();
    c.move_point(a, 1.0, 2.0).unwrap();
    let mut changed = 0;
    let n = allocations(|| {
        for i in 0..1000 {
            let before = c.value(w);
            let (x, y) = if i % 2 == 0 { (5.0, 6.0) } else { (1.0, 2.0) };
            let count = c.move_point(a, x, y).unwrap();
            changed += usize::from(count > 0 && c.value(w) != before);
        }
    });
    assert_eq!(changed, 1000, "every move must recompute `{watch}`");
    n
}

#[test]
fn a_midpoint_expression_recomputes_without_allocating() {
    let mut c = load("A = (1, 2)\nB = (3, 4)\nM = (A + B) / 2").unwrap();
    assert_eq!(drag(&mut c, "M"), 0);
}

#[test]
fn a_set_number_times_a_literal_recomputes_without_allocating() {
    let mut c = load("a = 0.3\nA = (1, 2)\nc = 0.5 a + x(A)").unwrap();
    // setting `a` drops its decimal: `0.5 a` is plain arithmetic again
    c.set_number(id(&c, "a"), 0.7).unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
}

#[test]
fn small_decimals_recompute_without_allocating() {
    // `0.5 a + 0.25` is exact on every recompute
    let mut c = load("a = 0.1\nA = (1, 2)\nc = 0.5 a + 0.25 + x(A)\nd = (a + 0.2) y(A)").unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
    assert_eq!(drag(&mut c, "d"), 0);
}

#[test]
fn a_function_value_recomputes_without_allocating() {
    let mut c = load("a = 1\nf(x) = x^2 + a\nv = f(2)").unwrap();
    let (a, v) = (id(&c, "a"), id(&c, "v"));
    c.set_number(a, 2.0).unwrap();
    let mut changed = 0;
    let n = allocations(|| {
        for i in 0..1000 {
            let before = c.value(v);
            let count = c.set_number(a, if i % 2 == 0 { 3.0 } else { 2.0 }).unwrap();
            changed += usize::from(count > 0 && c.value(v) != before);
        }
    });
    assert_eq!(changed, 1000);
    assert_eq!(n, 0);
}

#[test]
fn sampling_a_function_does_not_allocate() {
    let c = load("a = 0.1\nf(x) = x^2 + a\ng(x) = round(f(x) 0.1) + sin(x) 1.5").unwrap();
    for name in ["f", "g"] {
        let f = id(&c, name);
        let mut sum = 0.0;
        let n = allocations(|| {
            for _ in 0..10 {
                for i in 0..100 {
                    sum += c.function_value(f, f64::from(i) * 0.1 - 5.0).unwrap();
                }
            }
        });
        assert!(sum.is_finite());
        assert_eq!(n, 0, "sampling `{name}`");
    }
    assert!(matches!(c.value(id(&c, "a")), Value::Number(_)));
}

#[test]
fn rounding_a_literal_product_recomputes_without_allocating() {
    // the product of a literal and an object keeps the literal's class, so
    // `round` makes its result exact (`setPrecise`)
    let mut c =
        load("a = 0.3\nA = (1.234, 2)\nc = round(0.5 x(A), 2)\nd = round(2.54 a x(A), 3)").unwrap();
    c.set_number(id(&c, "a"), 0.7).unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
    assert_eq!(drag(&mut c, "d"), 0);
}

/// Moves `A` between `(x1, 2)` and `(x2, 2)` 1000 times; every move must
/// change `watch`. Returns the allocations of the 1000 moves.
fn drag_x(c: &mut Construction, watch: &str, x1: f64, x2: f64) -> u64 {
    let (a, w) = (id(c, "A"), id(c, watch));
    c.move_point(a, x2, 2.0).unwrap();
    c.move_point(a, x1, 2.0).unwrap();
    let mut changed = 0;
    let n = allocations(|| {
        for i in 0..1000 {
            let before = c.value(w);
            let x = if i % 2 == 0 { x2 } else { x1 };
            let count = c.move_point(a, x, 2.0).unwrap();
            changed += usize::from(count > 0 && c.value(w) != before);
        }
    });
    assert_eq!(changed, 1000, "every move must recompute `{watch}`");
    n
}

#[test]
fn an_exact_power_recomputes_without_allocating() {
    // the working precision of `1.5^2` depends on the digits of the exponent
    let mut c = load("A = (1, 2)\nc = 1.5^2 + x(A)\nd = 0.3^12 + y(A)").unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
    assert_eq!(drag(&mut c, "d"), 0);
}

#[test]
fn the_exact_decimal_of_a_rounded_double_does_not_allocate() {
    // `round(0.5 x(A), 2)` is exact as a double (`setPrecise`): 0.65 and
    // 0.85 have about 50 digits, which the next `+ 0.1` writes out
    let mut c =
        load("A = (1.3, 2)\nc = round(0.5 x(A), 2) + 0.1\nd = round(0.5 x(A), 2) * 3").unwrap();
    assert_eq!(drag_x(&mut c, "c", 1.3, 1.7), 0);
    assert_eq!(drag_x(&mut c, "d", 1.3, 1.7), 0);
    // coordinates in the usual range of a drawing
    let mut c = load("A = (1.3, 2)\nc = round(x(A) / 7, 6) - 0.25").unwrap();
    assert_eq!(drag_x(&mut c, "c", 123.457, -0.0123), 0);
}

#[test]
fn a_long_division_does_bounded_work() {
    // a quotient of 34 digits by a 200-digit divisor: a handful of buffers,
    // not one per subtraction
    let divisor = format!("1.{}7", "3".repeat(198));
    let mut c = load(&format!("A = (1, 2)\nc = 7 / {divisor} + x(A)")).unwrap();
    let n = drag(&mut c, "c");
    assert!(n <= 1000 * 8, "{n} allocations for 1000 divisions");
}

#[test]
fn a_refused_product_copies_nothing() {
    // 5000 by 5000 digits is past the work bound: the product is refused
    // before its operands are copied, and computed with doubles
    let long = "7".repeat(5000);
    let mut c = load(&format!("A = (1, 2)\nc = 0.{long} * 0.{long} + x(A)")).unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
}

#[test]
fn a_long_literal_recomputes_without_allocating() {
    let mut c =
        load("A = (1, 2)\nc = 3.14159265358979323846264338327950288419716939937510 x(A)").unwrap();
    assert_eq!(drag(&mut c, "c"), 0);
}
