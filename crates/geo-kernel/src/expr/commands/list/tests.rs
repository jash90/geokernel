use crate::expr::tests::{elems, flat, num, rebuilt};
use crate::kernel::{Construction, Kind, Value};
use crate::script::{exec, load};

#[test]
fn list_literals_hold_numbers_points_booleans_and_lists() {
    let mut c = load(
        "a = 2\n\
         A = (1, 2)\n\
         v = (1, 1)\n\
         l = {1, 2, 3}\n\
         m = {}\n\
         n = {0.1 + 0.2, 1 / 0, sqrt(-1)}\n\
         p = {(1, 2), A, v, A + v, 2 v}\n\
         q = {{1, 2}, {3}, {}}\n\
         r = {a < 3, true}\n\
         s = {a, 2 a, a^2}",
    )
    .unwrap();
    for label in ["l", "m", "n", "p", "q", "r", "s"] {
        assert_eq!(c.kind(c.id(label).unwrap()), Kind::List, "{label}");
    }
    assert_eq!(c.value(c.id("l").unwrap()), Value::List { len: 3 });
    assert_eq!(elems(&c, "l"), "{1, 2, 3}");
    assert_eq!(elems(&c, "m"), "{}");
    // an element is computed with the exact decimals of its literals, then
    // kept as a double; a value that is not finite is an undefined number
    assert_eq!(elems(&c, "n"), "{0.3, ?number, ?number}");
    // a tuple is a point; an object keeps its kind; an operation with a
    // point in it is a point
    assert_eq!(elems(&c, "p"), "{(1, 2), (1, 2), v(1, 1), (2, 3), v(2, 2)}");
    assert_eq!(elems(&c, "q"), "{{1, 2}, {3}, {}}");
    assert_eq!(elems(&c, "r"), "{true, true}");
    assert_eq!(elems(&c, "s"), "{2, 4, 4}");
    // a constant list is an expression that never changes
    assert!(c.set_number(c.id("l").unwrap(), 1.0).is_err());
    rebuilt(&c);
}

#[test]
fn elements_are_picked_by_a_rounded_index() {
    let mut c = load(
        "n = 1\n\
         l = {10, 20, 30}\n\
         b = l(n)\n\
         d = l(-n)\n\
         g = Element(l, n)\n\
         h = Element(l, -n)\n\
         k = l(n) + 1\n\
         q = {{1, 2}, {3}}\n\
         r = q(n)\n\
         s = Length(q(n))",
    )
    .unwrap();
    let n = c.id("n").unwrap();
    let pick = |c: &Construction| ["b", "d", "g", "h", "k"].map(|l| c.value(c.id(l).unwrap()));
    let u = Value::Undefined;
    let v = Value::Number;
    assert_eq!(pick(&c), [v(10.0), v(30.0), v(10.0), u, v(11.0)]);
    assert_eq!(elems(&c, "r"), "{1, 2}");
    assert_eq!(num(&c, "s"), 2.0);
    // `l(-1)` counts from the end; `Element` takes no negative index
    c.set_number(n, -1.0).unwrap();
    assert_eq!(pick(&c), [v(30.0), v(10.0), u, v(10.0), v(31.0)]);
    // rounded half up
    c.set_number(n, 2.5).unwrap();
    assert_eq!(pick(&c), [v(30.0), v(20.0), v(30.0), u, v(31.0)]);
    c.set_number(n, -2.5).unwrap();
    assert_eq!(pick(&c)[0], v(20.0));
    // 0 and past the end are undefined, and so is what depends on them
    for bad in [0.0, 4.0, -4.0] {
        c.set_number(n, bad).unwrap();
        assert_eq!(pick(&c)[0], u, "{bad}");
        assert_eq!(pick(&c)[4], u, "{bad}");
    }
    // indices beyond the integer range are undefined, not an overflow
    for huge in [-1e300, 1e300] {
        c.set_number(n, huge).unwrap();
        assert_eq!(pick(&c), [u; 5], "{huge}");
    }
    c.set_number(n, 2.0).unwrap();
    assert_eq!(elems(&c, "r"), "{3}");
    assert_eq!(num(&c, "s"), 1.0);
    rebuilt(&c);
    c.set_number(n, 1.0).unwrap();
    assert_eq!(pick(&c), [v(10.0), v(30.0), v(10.0), u, v(11.0)]);
}

#[test]
fn length_and_sum_read_the_elements_as_doubles() {
    let mut c = load(
        "n = 2\n\
         a = 0.1\n\
         l = {a, 0.2, 0.3}\n\
         b = Length(l)\n\
         d = Length({})\n\
         g = Sum(l)\n\
         h = Sum({})\n\
         k = Sum(l, n)\n\
         m = Sum({a < 1, true})\n\
         p = Sum({1 / 0, 1})\n\
         q = Length(Sequence(n))",
    )
    .unwrap();
    assert_eq!(num(&c, "b"), 3.0);
    assert_eq!(num(&c, "d"), 0.0);
    // a plain sum of doubles: 0.1 + 0.2 + 0.3 is not 0.6
    assert_eq!(num(&c, "g"), 0.1 + 0.2 + 0.3);
    assert_eq!(num(&c, "h"), 0.0);
    assert_eq!(num(&c, "k"), 0.1 + 0.2);
    assert_eq!(num(&c, "m"), 2.0);
    assert_eq!(c.value(c.id("p").unwrap()), Value::Undefined);
    assert_eq!(num(&c, "q"), 2.0);
    let n = c.id("n").unwrap();
    // `Sum(l, n)` truncates `n`; 0 is an empty sum, past the end undefined
    c.set_number(n, 1.7).unwrap();
    assert_eq!(num(&c, "k"), 0.1);
    c.set_number(n, 0.0).unwrap();
    assert_eq!(num(&c, "k"), 0.0);
    assert_eq!(num(&c, "q"), 0.0);
    for bad in [4.0, -1.0] {
        c.set_number(n, bad).unwrap();
        assert_eq!(c.value(c.id("k").unwrap()), Value::Undefined, "{bad}");
    }
    c.set_number(c.id("a").unwrap(), 5.0).unwrap();
    assert_eq!(num(&c, "m"), 1.0);
    rebuilt(&c);
}

#[test]
fn sequence_has_four_forms() {
    let mut c = load(
        "a = 1\n\
         b = 4\n\
         s = 0.5\n\
         A = (1, 2)\n\
         l = Sequence(b)\n\
         m = Sequence(a, b)\n\
         n = Sequence(a, b, s)\n\
         p = Sequence(k^2, k, a, b)\n\
         q = Sequence(k, k, a, b, s)\n\
         r = Sequence(A + (k, 0), k, 0, b)\n\
         t = Sequence(Sequence(j + k, j, 1, k), k, a, 3)\n\
         u = Sequence(0.1, 1, 0.1)\n\
         w = Sum(Sequence(k, k, 1, b))",
    )
    .unwrap();
    assert_eq!(elems(&c, "l"), "{1, 2, 3, 4}");
    assert_eq!(elems(&c, "m"), "{1, 2, 3, 4}");
    assert_eq!(elems(&c, "n"), "{1, 1.5, 2, 2.5, 3, 3.5, 4}");
    assert_eq!(elems(&c, "p"), "{1, 4, 9, 16}");
    assert_eq!(elems(&c, "q"), "{1, 1.5, 2, 2.5, 3, 3.5, 4}");
    assert_eq!(elems(&c, "r"), "{(1, 2), (2, 2), (3, 2), (4, 2), (5, 2)}");
    assert_eq!(elems(&c, "t"), "{{2}, {3, 4}, {4, 5, 6}}");
    // decimal steps are exact
    assert_eq!(
        elems(&c, "u"),
        "{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1}"
    );
    assert_eq!(num(&c, "w"), 10.0);
    let (a, b, s) = (c.id("a").unwrap(), c.id("b").unwrap(), c.id("s").unwrap());
    // a range counts down; a count below 1 is empty
    c.set_number(a, 4.0).unwrap();
    c.set_number(b, 1.5).unwrap();
    assert_eq!(elems(&c, "m"), "{4, 3, 2}");
    assert_eq!(elems(&c, "p"), "{}");
    assert_eq!(elems(&c, "l"), "{1, 2}");
    c.set_number(b, -3.0).unwrap();
    assert_eq!(elems(&c, "l"), "{}");
    // a step of the wrong sign: `AlgoSequenceRange` is undefined,
    // `AlgoSequence` empty; a zero step is undefined in both
    c.set_number(a, 1.0).unwrap();
    c.set_number(b, 4.0).unwrap();
    c.set_number(s, -0.5).unwrap();
    assert_eq!(c.value(c.id("n").unwrap()), Value::Undefined);
    assert_eq!(elems(&c, "q"), "{}");
    c.set_number(s, 0.0).unwrap();
    assert_eq!(c.value(c.id("n").unwrap()), Value::Undefined);
    assert_eq!(c.value(c.id("q").unwrap()), Value::Undefined);
    rebuilt(&c);
    // and back
    c.set_number(s, 0.5).unwrap();
    assert_eq!(elems(&c, "q"), "{1, 1.5, 2, 2.5, 3, 3.5, 4}");
    assert_eq!(elems(&c, "t"), "{{2}, {3, 4}, {4, 5, 6}}");
}

#[test]
fn list_arithmetic_works_element_by_element() {
    let mut c = load(
        "a = 2\n\
         A = (1, 2)\n\
         B = (3, -1)\n\
         v = (1, 1)\n\
         l = {1, 2, 3}\n\
         m = {a, 3, 1}\n\
         b = l + m\n\
         d = 2 l + 1\n\
         g = l^2\n\
         h = -l\n\
         k = sin(l - l)\n\
         n = l + {1, 2}\n\
         p = {A, B} + v\n\
         q = 2 {A, B}\n\
         r = {v, 2 v} + v\n\
         s = x({A, B})\n\
         t = {a, 2} - A\n\
         T = {A, B} - A",
    )
    .unwrap();
    // a list minus a point is no list: the list is read as `(x, y)` when
    // it has two numbers, and the result is undefined otherwise
    assert_eq!(
        c.value(c.id("t").unwrap()),
        Value::Vector { x: 1.0, y: 0.0 }
    );
    assert_eq!(c.kind(c.id("T").unwrap()), Kind::Point);
    assert_eq!(c.value(c.id("T").unwrap()), Value::Undefined);
    assert_eq!(elems(&c, "b"), "{3, 5, 4}");
    assert_eq!(elems(&c, "d"), "{3, 5, 7}");
    assert_eq!(elems(&c, "g"), "{1, 4, 9}");
    assert_eq!(elems(&c, "h"), "{-1, -2, -3}");
    assert_eq!(elems(&c, "k"), "{0, 0, 0}");
    // lists of unequal length give an empty list
    assert_eq!(elems(&c, "n"), "{}");
    assert_eq!(elems(&c, "p"), "{(2, 3), (4, 0)}");
    assert_eq!(elems(&c, "q"), "{(2, 4), (6, -2)}");
    // a vector result in a list is a point
    assert_eq!(elems(&c, "r"), "{(2, 2), (3, 3)}");
    assert_eq!(elems(&c, "s"), "{1, 3}");
    c.set_number(c.id("a").unwrap(), 5.0).unwrap();
    c.move_point(c.id("A").unwrap(), -2.0, 5.0).unwrap();
    assert_eq!(elems(&c, "b"), "{6, 5, 4}");
    assert_eq!(elems(&c, "p"), "{(-1, 6), (4, 0)}");
    rebuilt(&c);
    c.set_number(c.id("a").unwrap(), 2.0).unwrap();
    c.move_point(c.id("A").unwrap(), 1.0, 2.0).unwrap();
    assert_eq!(elems(&c, "b"), "{3, 5, 4}");
    assert_eq!(elems(&c, "q"), "{(2, 4), (6, -2)}");
}

#[test]
fn a_list_object_minus_a_point_is_read_as_coordinates() {
    let mut c = load(
        "a = 2\n\
         A = (1, 2)\n\
         l = {a, 2}\n\
         t = l - A\n\
         u = A - l\n\
         c = x(l - A)",
    )
    .unwrap();
    let v = |c: &Construction, l: &str| c.value(c.id(l).unwrap());
    assert_eq!(v(&c, "t"), Value::Vector { x: 1.0, y: 0.0 });
    assert_eq!(v(&c, "u"), Value::Vector { x: -1.0, y: 0.0 });
    assert_eq!(num(&c, "c"), 1.0);
    c.set_number(c.id("a").unwrap(), 3.0).unwrap();
    assert_eq!(v(&c, "t"), Value::Vector { x: 2.0, y: 0.0 });
    assert_eq!(num(&c, "c"), 2.0);
    rebuilt(&c);
    // a function that reads a list object is refused like one with a list
    assert!(exec(&mut c, "f(x) = x + x(l - A)").is_err());
}

#[test]
fn if_picks_a_list_branch() {
    let mut c = load(
        "a = 1\n\
         l = If(a > 0, {1, 2})\n\
         n = Length(If(a > 0, {1, 2}))\n\
         m = If(a > 0, {a, 2}, {3})\n\
         k = If(a > 2, {1}, a > 0, {a, a, a}, {})\n\
         j = {If(a > 0, {a}, {0}), 5}",
    )
    .unwrap();
    assert_eq!(elems(&c, "l"), "{1, 2}");
    assert_eq!(num(&c, "n"), 2.0);
    assert_eq!(elems(&c, "m"), "{1, 2}");
    assert_eq!(elems(&c, "k"), "{1, 1, 1}");
    assert_eq!(elems(&c, "j"), "{{1}, 5}");
    let a = c.id("a").unwrap();
    c.set_number(a, -1.0).unwrap();
    assert_eq!(flat(&c, c.id("l").unwrap()), None);
    assert_eq!(c.value(c.id("n").unwrap()), Value::Undefined);
    assert_eq!(elems(&c, "m"), "{3}");
    assert_eq!(elems(&c, "k"), "{}");
    assert_eq!(elems(&c, "j"), "{{0}, 5}");
    rebuilt(&c);
    c.set_number(a, 3.0).unwrap();
    assert_eq!(elems(&c, "l"), "{1, 2}");
    assert_eq!(num(&c, "n"), 2.0);
    assert_eq!(elems(&c, "k"), "{1}");
    // a list and a number do not fit together as branches
    assert!(exec(&mut c, "b = If(a > 0, {1}, 2)").is_err());
}

#[test]
fn an_undefined_list_makes_its_dependents_undefined() {
    let mut c = load(
        "s = 1\n\
         l = Sequence(k, k, 1, 3, s)\n\
         m = l + 1\n\
         b = Sum(l)\n\
         d = l(1)\n\
         g = {l, 1}\n\
         h = Length(l)",
    )
    .unwrap();
    assert_eq!(elems(&c, "m"), "{2, 3, 4}");
    assert_eq!(elems(&c, "g"), "{{1, 2, 3}, 1}");
    c.set_number(c.id("s").unwrap(), 0.0).unwrap();
    for label in ["l", "m", "b", "d", "h"] {
        assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined, "{label}");
    }
    // an undefined list inside a literal is an undefined element
    assert_eq!(elems(&c, "g"), "{?list, 1}");
    rebuilt(&c);
    c.set_number(c.id("s").unwrap(), 1.0).unwrap();
    assert_eq!(num(&c, "b"), 6.0);
    assert_eq!(elems(&c, "g"), "{{1, 2, 3}, 1}");
}

#[test]
fn a_list_past_the_element_cap_is_undefined() {
    let mut c = load("n = 3\nl = Sequence(n)\nm = Sequence(Sequence(n), k, 1, n)").unwrap();
    let n = c.id("n").unwrap();
    c.set_number(n, 2_000_000.0).unwrap();
    assert_eq!(c.value(c.id("l").unwrap()), Value::Undefined);
    // 1100 lists of 1100 elements: more than 2^20 elements in all
    c.set_number(n, 1100.0).unwrap();
    assert_eq!(c.value(c.id("m").unwrap()), Value::Undefined);
    c.set_number(n, 3.0).unwrap();
    assert_eq!(elems(&c, "l"), "{1, 2, 3}");
    assert_eq!(elems(&c, "m"), "{{1, 2, 3}, {1, 2, 3}, {1, 2, 3}}");
}

#[test]
fn misplaced_lists_are_errors() {
    let base = "a = 2\nA = (1, 2)\nl = {1, 2}";
    for line in [
        "b = {1, 2",
        "b = {1, , 2}",
        "b = l < 3",
        "b = l == l",
        "b = {true} && true",
        "f(x) = {x, 1}",
        "f(x) = x + l(1)",
        "f(x) = Sum({x})",
        "b = (l, 1)",
        "b = Sequence(k, k, 1)",
        "b = Sequence(k, 2, 1, 3)",
        "b = Sequence(k, k, 1, 2, 3, 4)",
        "b = Sum(l, l)",
        "b = Length(1, 2)",
        "b = l(1, 2)",
        "b = If(a < 3, l, 1)",
    ] {
        let mut c = load(base).unwrap();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), 3, "`{line}` left an object behind");
    }
}

#[test]
fn list_statistics_and_reordering() {
    let mut c = load(
        "n = 2\n\
         a = 3\n\
         A = (1, 2)\n\
         B = (3, -1)\n\
         l = {a, 1, 2, -a}\n\
         b = Product(l)\n\
         d = Product(l, n)\n\
         g = Min(l) + Max(l)\n\
         h = Mean(l)\n\
         k = Min(a, 1)\n\
         P = Sum({A, B})\n\
         p = Reverse(l)\n\
         q = First(l, n)\n\
         r = Last(l, n)\n\
         s = Sort(l)\n\
         t = Sort({B, A, (1, 0)})\n\
         u = Append(l, n)\n\
         w = Append(n, {A})\n\
         m = Join(l, {n}, {})\n\
         o = Join({{1}, {2, a}})",
    )
    .unwrap();
    assert_eq!(num(&c, "b"), -18.0);
    assert_eq!(num(&c, "d"), 3.0);
    assert_eq!(num(&c, "g"), 0.0);
    assert_eq!(num(&c, "h"), 0.75);
    assert_eq!(num(&c, "k"), 1.0);
    assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 4.0, y: 1.0 });
    assert_eq!(elems(&c, "p"), "{-3, 2, 1, 3}");
    assert_eq!(elems(&c, "q"), "{3, 1}");
    assert_eq!(elems(&c, "r"), "{2, -3}");
    assert_eq!(elems(&c, "s"), "{-3, 1, 2, 3}");
    assert_eq!(elems(&c, "t"), "{(1, 0), (1, 2), (3, -1)}");
    assert_eq!(elems(&c, "u"), "{3, 1, 2, -3, 2}");
    assert_eq!(elems(&c, "w"), "{2, (1, 2)}");
    assert_eq!(elems(&c, "m"), "{3, 1, 2, -3, 2}");
    assert_eq!(elems(&c, "o"), "{1, 2, 3}");
    let n = c.id("n").unwrap();
    // a count cut to a whole number; 0 is empty, past the end undefined;
    // `Product` of no elements is undefined
    c.set_number(n, 0.0).unwrap();
    assert_eq!(elems(&c, "q"), "{}");
    assert_eq!(c.value(c.id("d").unwrap()), Value::Undefined);
    c.set_number(n, 3.9).unwrap();
    assert_eq!(elems(&c, "r"), "{1, 2, -3}");
    for bad in [5.0, -1.0] {
        c.set_number(n, bad).unwrap();
        assert_eq!(c.value(c.id("q").unwrap()), Value::Undefined, "{bad}");
        assert_eq!(c.value(c.id("d").unwrap()), Value::Undefined, "{bad}");
    }
    c.set_number(c.id("a").unwrap(), 0.5).unwrap();
    assert_eq!(elems(&c, "s"), "{-0.5, 0.5, 1, 2}");
    rebuilt(&c);
    for line in [
        "bad = Product({A})",
        "bad = Sort({1, 2}, 3)",
        "bad = Append({1})",
        "bad = Join({1}, 2)",
        "bad = Join({1, 2})",
        "bad = First({1}, A)",
    ] {
        let len = c.len();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), len, "`{line}` left an object behind");
    }
}

/// The evaluation work `f` does on this thread (see `work_done`).
fn work_of<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let before = crate::expr::work_done();
    let r = f();
    (r, crate::expr::work_done() - before)
}

/// The most work one evaluation past the element bound does here: at most
/// [`crate::expr::MAX_ELEMENTS`] elements built, each with a step of counting,
/// one of filling and a small body (up to 3 nodes visited, 3 units each),
/// and a range counted within what is left.
const WORK_BOUND: u64 = 12 * crate::expr::MAX_ELEMENTS;

fn is_defined(c: &Construction, label: &str) -> bool {
    match c.value(c.id(label).unwrap()) {
        Value::Undefined => false,
        Value::Number(v) => !v.is_nan(),
        _ => true,
    }
}

#[test]
fn the_nested_sequence_of_the_fuzz_corpus_stops_at_the_bound() {
    // `lists-seq-011` with `b = 4` mutated to `b = 1E5`: about 5e9 elements
    // in all, each inner list counted and built for every outer value
    let script = |b: &str| {
        format!(
            "a = 1\nb = {b}\ns = 0.5\nA = (1, 2)\n\
             l = Sequence(Sequence(j + k, j, 1, k), k, a, b)"
        )
    };
    // created at the size
    let (c, work) = work_of(|| load(&script("1E5")).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= WORK_BOUND, "creation did {work} units of work");
    // recomputed at the size
    let mut c = load(&script("4")).unwrap();
    assert_eq!(elems(&c, "l"), "{{2}, {3, 4}, {4, 5, 6}, {5, 6, 7, 8}}");
    let b = c.id("b").unwrap();
    let (_, work) = work_of(|| c.set_number(b, 1e5).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= WORK_BOUND, "the update did {work} units of work");
    c.set_number(b, 4.0).unwrap();
    assert_eq!(elems(&c, "l"), "{{2}, {3, 4}, {4, 5, 6}, {5, 6, 7, 8}}");
}

#[test]
fn the_loop_around_a_list_past_the_bound_stops_at_once() {
    // the outer range fits, its first element does not: no more elements
    // are evaluated after it, so the work is the outer count and little else
    let mut c = load("n = 3\nl = Sequence(Sum(Sequence(2E6)), k, 1, n)").unwrap();
    let n = c.id("n").unwrap();
    let (_, work) = work_of(|| c.set_number(n, 1e6).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= 1_000_100, "{work} units of work");
}

/// Creates `l = expr` at `small` (3 when `None`), checks it is defined
/// exactly when `small` is given, then sets `n` to `large` and checks that
/// `l` is undefined and the update did at most `bound` units of work (`m`
/// is recomputed too), and that `l` is defined again back at `small`.
fn check_over_bound(expr: &str, small: Option<f64>, large: f64, bound: u64) {
    let mut c = load(&format!("n = 3\nf(x) = x^2 + 1\n{COSTLY}\nm = Sequence(n)")).unwrap();
    let n = c.id("n").unwrap();
    c.set_number(n, small.unwrap_or(3.0)).unwrap();
    exec(&mut c, &format!("l = {expr}")).unwrap();
    assert_eq!(
        is_defined(&c, "l"),
        small.is_some(),
        "`{expr}` at {small:?}"
    );
    let (_, work) = work_of(|| c.set_number(n, large).unwrap());
    assert!(!is_defined(&c, "l"), "`{expr}` at n = {large}");
    assert!(
        work <= bound,
        "`{expr}` at n = {large}: {work} units of work"
    );
    if let Some(small) = small {
        c.set_number(n, small).unwrap();
        assert!(is_defined(&c, "l"), "`{expr}` back at n = {small}");
    }
}

/// Lists past the element bound in a loop: before the bound stopped
/// evaluations, each of these went on counting every inner range in full
/// (about 5e9 units at `n = 1E5`).
const OVER_ELEMENT_BOUND: &[(&str, Option<f64>, f64)] = &[
    (
        "Sequence(Sum(Sequence(j, j, 1, k)), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Product(Sequence(j, j, 1, k)), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Join(Sequence(Sequence(j, j, 1, k), k, 1, n))",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Sequence(j, j, 1, k) * 2, k, 1, n)",
        Some(3.0),
        1e5,
    ),
    ("Sequence(-Sequence(j, j, 1, k), k, 1, n)", Some(3.0), 1e5),
    (
        "Sequence(Sequence(j, j, 1, k) + Sequence(j, j, 1, k), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(First(Sequence(j, j, 1, k), k), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Last(Sequence(j, j, 1, k), 1E9), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Reverse(Sequence(j, j, 1, k)), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Sort(Sequence(-j, j, 1, k)), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Append(Sequence(j, j, 1, k), 0), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(Element(Sequence(j, j, 1, k), 1), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    ("Sequence(Sequence(1, k), k, 1, n)", Some(3.0), 1e5),
    ("Sequence(Sequence(0, k, 0.5), k, 1, n)", Some(3.0), 1e5),
    (
        "If(n > 0, Sequence(Sequence(j + k, j, 1, k), k, 1, n), {1})",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(If(k > 0, Sequence(j, j, 1, k), {0}), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(If(k > 0, Sum(Sequence(j, j, 1, k)), 0), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    (
        "Sequence(f(Sum(Sequence(j, j, 1, k))), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    ("{Sequence(Sequence(j, j, 1, k), k, 1, n)}", Some(3.0), 1e5),
];

/// Lists that are undefined at the large size without a loop going on past
/// the bound: they were cheap before the bound stopped evaluations too, and
/// are kept so that every list construct and every kind of range (a zero,
/// tiny, stalled or negative step, NaN and infinite bounds, `1/0` being
/// infinite) is checked for its result and its work. `None` marks a range
/// that never ends (within `MIN_PRECISION` of its end, or stalled) or a
/// count that is always too large.
const UNDEFINED_WITHOUT_A_LOOP: &[(&str, Option<f64>, f64)] = &[
    ("Sum(Sequence(k, k, 1, n))", Some(3.0), 1e7),
    ("Sum(Sequence(n), 2)", Some(3.0), 1e7),
    ("Product(Sequence(n))", Some(3.0), 1e7),
    ("Mean(Sequence(n))", Some(3.0), 1e7),
    ("Max(Sequence(k^2, k, 1, n))", Some(3.0), 1e7),
    ("Length(Sequence(n))", Some(3.0), 1e7),
    (
        "Sequence(Join({Sequence(k), Sequence(j, j, 1, k)}), k, 1, n)",
        Some(3.0),
        1e5,
    ),
    ("Sequence(n) + Sequence(n)", Some(3.0), 7e5),
    ("First(Sequence(n), 1E9)", None, 1e7),
    ("First(Sequence(n), n - 1)", Some(3.0), 1e7),
    ("Sequence(j, j, 0, n, 1E-7)", Some(0.0), 1.0),
    ("Sequence(j, j, n, 0, -1E-7)", Some(0.0), 1.0),
    ("Sequence(0, n, 1E-7)", Some(0.0), 1.0),
    ("Sequence(j, j, -n, n)", Some(3.0), 1e300),
    ("Sequence(j, j, 1, n, 0)", Some(1.0), 3.0),
    ("Sequence(1, n, 0)", None, 5.0),
    ("Sequence(j, j, 1, sqrt(-n))", Some(0.0), 3.0),
    ("Sequence(j, j, 1, 1 / n)", Some(1.0), 0.0),
    ("Sequence(j, j, 1 / n, 1 / n)", Some(1.0), 0.0),
    ("Sequence(j, j, -1 / n, 1 / n)", Some(1.0), 0.0),
    ("Sequence(1 / n, 3)", Some(1.0), 0.0),
    ("Sequence(j, j, 0, n, 1E-300)", None, 1.0),
    ("Sequence(j, j, n, 0, -1E-300)", None, 1.0),
    ("Sequence(j, j, 1E300, 1E300 + n, 1)", None, 0.0),
    ("Sequence(0, n, 1E-300)", None, 1.0),
    ("Sequence(1E15, 1E15 + n, 1E-9)", None, 1.0),
    ("Sequence(-n, n)", Some(3.0), 1e15),
    ("{Sequence(n), Sequence(n)}", Some(3.0), 7e5),
];

/// Lists within the element bound whose bodies cost more than the work
/// bound allows: a costly function call, exact decimals of 3001 digits,
/// short exact divisions. Before the work bound, the first took about
/// 0.64 ms of a release build for each value, the product about 48 us, the
/// quotient about 110 us and the sum of quotients about 7 us (7.4 s for the
/// whole list).
const OVER_WORK_BOUND: &[(&str, Option<f64>, f64)] = &[
    ("Sequence(g(k), k, 1, n)", Some(30.0), 1e4),
    ("Sequence(k + g(k), k, 1, n)", Some(30.0), 1e4),
    (
        "Sequence(Sum(Sequence(g(j), j, 1, k)), k, 1, n)",
        Some(3.0),
        1e3,
    ),
    (
        "Sequence((1 + 1E-3000) * (1 + 1E-3000), k, 1, n)",
        Some(1000.0),
        1e5,
    ),
    ("Sequence(1 / (1 + 1E-3000), k, 1, n)", Some(300.0), 1e5),
    (
        "Sequence(1 / 3 + 1 / 7 + 1 / 9 + 1 / 11 + 1 / 13 + 1 / 17 + 1 / 19, k, 1, n)",
        Some(1000.0),
        1048576.0,
    ),
    ("Sequence((1 + 1E-70) / 7, k, 1, n)", Some(1000.0), 1e6),
];

/// A function `g(x) = 65536 x` whose tree, functions expanded, has about
/// 240 000 nodes (the review's example): 6.4 s of a release build for 10^4
/// values before the work bound.
const COSTLY: &str = "h(x) = x + x + x + x\ng(x) = h(h(h(h(h(h(h(h(x))))))))";

/// The most work one evaluation past the work bound may do.
const PAST_WORK_BOUND: u64 = crate::expr::MAX_EVALUATION_WORK + crate::expr::MAX_OVERRUN;

#[test]
fn a_costly_sequence_body_stops_at_the_work_bound() {
    for &(expr, small, large) in OVER_WORK_BOUND {
        check_over_bound(expr, small, large, PAST_WORK_BOUND);
    }
    // created at the size, and the values within it
    let (c, work) = work_of(|| load(&format!("{COSTLY}\nl = Sequence(g(k), k, 1, 1E4)")).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= PAST_WORK_BOUND, "creation did {work} units of work");
    let c = load(&format!("{COSTLY}\nl = Sequence(g(k), k, 1, 3)")).unwrap();
    assert_eq!(elems(&c, "l"), "{65536, 131072, 196608}");
}

#[test]
fn a_full_length_list_with_a_small_body_is_defined() {
    // 2^20 values, each a step of counting, one of filling and the nodes of
    // the body visited (3 units each): within `MAX_EVALUATION_WORK` for 10
    // terms (59 units a value), past it for 15 (89 units, 0.35 s)
    let sum = |terms: usize| vec!["k"; terms].join(" + ");
    let c = load(&format!("l = Sequence({}, k, 1, 1048576)", sum(10))).unwrap();
    assert_eq!(c.value(c.id("l").unwrap()), Value::List { len: 1 << 20 });
    let (c, work) = work_of(|| load(&format!("l = Sequence({}, k, 1, 1048576)", sum(15))).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= PAST_WORK_BOUND, "{work} units of work");
}

/// A polynomial of degree 20.
const POLYNOMIAL: &str = "p(x) = 1 + x + x^2 + x^3 + x^4 + x^5 + x^6 + x^7 + x^8 + x^9 + x^10 \
                          + x^11 + x^12 + x^13 + x^14 + x^15 + x^16 + x^17 + x^18 + x^19 + x^20";

/// Ordinary lists that take from under a millisecond to 0.3 s of a release
/// build (`1.0000001^999999` is 6 200 units for each value, `1.0001^365`
/// 2 500), and the most work each may do: what it does now, rounded up. Charging the static cost of a body for each value, with
/// both branches of an `If` and a sampled function's body for each call
/// around it, charged the first five 6 to 300 times as much (31 to 62
/// million units) and, with a bound of 2^25, made them undefined.
fn defined_within_the_bound() -> Vec<(String, u64)> {
    let logistic = |levels: usize, n: usize| {
        format!(
            "f(x) = 3.7 x (1 - x)\nl = Sequence({}k / 10000{}, k, 1, {n})",
            "f(".repeat(levels),
            ")".repeat(levels)
        )
    };
    vec![
        (logistic(9, 10000), 1_100_000),
        (logistic(12, 1000), 150_000),
        (
            format!("{POLYNOMIAL}\nl = Sum(Sequence(p(p(k / 10000)), k, 1, 11000))"),
            5_500_000,
        ),
        (
            "N(x) = x - (x^2 - 2) / (2x)\nl = Sequence(N(N(N(N(N(N(k / 100)))))), k, 1, 10000)"
                .into(),
            1_500_000,
        ),
        (
            format!(
                "{POLYNOMIAL}\nq(x) = p(p(x))\nl = Sequence(If(k < 3, q(k / 10000), 0), k, 1, 10000)"
            ),
            200_000,
        ),
        (
            "l = Sequence(1.0000001^999999 + k, k, 1, 10000)".into(),
            70_000_000,
        ),
        (
            "l = Sequence(k * 1.0001^365, k, 1, 29000)".into(),
            crate::expr::MAX_EVALUATION_WORK,
        ),
        // a list object read in a body is read in place, as upstream reads
        // the `GeoList`: neither copied nor counted against the element
        // bound for each value (each read was, and these were undefined
        // from about 2 000 values on)
        (list_read(10000, "L(k) + 1"), 340_000),
        (list_read(10000, "Element(L, k) + 1"), 340_000),
        (list_read(10000, "Length(L) + k"), 310_000),
        (list_read(3000, "Max(L) + k"), 9_100_000),
        (list_read(3000, "Sum(First(L, k))"), 4_600_000),
        // in the table of lists past the element bound before
        (
            "L = Sequence(100000)\nl = Sequence(Length(L), k, 1, 100000)".into(),
            3_500_000,
        ),
    ]
}

/// `l` reading the list object `L` of `n` elements in a body of `n` values.
fn list_read(n: usize, body: &str) -> String {
    format!("L = Sequence(k^2, k, 1, {n})\nl = Sequence({body}, k, 1, {n})")
}

/// The value of `l`: its elements or its number.
fn value_of_l(c: &Construction) -> String {
    let id = c.id("l").unwrap();
    flat(c, id).unwrap_or_else(|| format!("{:?}", c.value(id)))
}

#[test]
fn ordinary_lists_are_defined_with_their_unbounded_value() {
    for (script, most) in defined_within_the_bound() {
        let (c, work) = work_of(|| load(&script).unwrap());
        assert!(is_defined(&c, "l"), "{script}");
        assert!(work <= most, "{script}: {work} units of work");
        let free = crate::expr::eval::unbounded(|| load(&script).unwrap());
        assert_eq!(value_of_l(&c), value_of_l(&free), "{script}");
    }
    // the evaluation charges the branch of `If` it takes: the other one
    // would be 10^4 times as much
    let script = format!("{POLYNOMIAL}\nl = Sequence(If(k < 3, p(p(k / 10000)), 0), k, 1, 10000)");
    let (_, taken) = work_of(|| load(&script).unwrap());
    let script = format!("{POLYNOMIAL}\nl = Sequence(If(k > 3, p(p(k / 10000)), 0), k, 1, 10000)");
    let (_, both) = work_of(|| load(&script).unwrap());
    assert!(taken * 20 < both, "{taken} and {both} units of work");
}

#[test]
fn a_list_past_the_work_bound_is_undefined() {
    // `k * 1.0001^365` costs about 2 520 units for each value: undefined
    // from about 31 700 values on
    let (c, work) = work_of(|| load("l = Sequence(k * 1.0001^365, k, 1, 33000)").unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= PAST_WORK_BOUND, "{work} units of work");
}

#[test]
fn a_range_upstream_refuses_is_an_undefined_element() {
    // `AlgoSequenceRange` past `LARGEST_INTEGER` and `AlgoSequence` with an
    // infinite count are undefined lists, elements of the list around them
    let c = load(
        "a = {Sequence(1, 1E20), {1}}\n\
         b = {Sequence(k, k, 0, 1, 1E-320), {1}}",
    )
    .unwrap();
    assert_eq!(elems(&c, "a"), "{?list, {1}}");
    assert_eq!(elems(&c, "b"), "{?list, {1}}");
    // and counted like any other element: a million of them fit
    let (c, work) = work_of(|| load("l = Sequence(Sequence(1, 1E20), k, 1, 1E6)").unwrap());
    assert_eq!(c.value(c.id("l").unwrap()), Value::List { len: 1_000_000 });
    assert!(
        (2_000_000..=WORK_BOUND * 2).contains(&work),
        "{work} units of work"
    );
    // `Sequence(n)` counts to `(int) Math.round(n)`: past `int` it wraps,
    // and a count of 0 or less is empty
    let c = load(
        "a = Sequence(1E20)\n\
         b = Sequence(3E9)\n\
         d = Sequence(2^32 + 3)\n\
         e = Sequence(-5)\n\
         g = Sequence(5E9)",
    )
    .unwrap();
    assert_eq!(elems(&c, "a"), "{}");
    assert_eq!(elems(&c, "b"), "{}");
    assert_eq!(elems(&c, "d"), "{1, 2, 3}");
    assert_eq!(elems(&c, "e"), "{}");
    // 705032704 values: past the element bound
    assert!(!is_defined(&c, "g"));
}

#[test]
fn every_list_construct_stops_at_the_element_bound() {
    for &(expr, small, large) in OVER_ELEMENT_BOUND.iter().chain(UNDEFINED_WITHOUT_A_LOOP) {
        check_over_bound(expr, small, large, 2 * WORK_BOUND);
    }
}

/// A number of `n` digits, none of them zero: `1.` and `n - 1` digits from
/// a fixed sequence.
fn dense_digits(n: usize, seed: u64) -> String {
    let mut x = seed;
    let mut s = String::from("1.");
    for _ in 1..n {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        s.push(char::from(b'1' + ((x >> 33) % 9) as u8));
    }
    s
}

#[test]
fn long_numbers_with_dense_digits_cost_what_they_take() {
    // a 99 000-digit number times 3 took 0.29 s of a release build for each
    // value (a carry pass over the whole product every 64 digits) and was
    // charged 0.4 ms; it is 0.2 ms and about 103 000 units now
    let a = dense_digits(99_000, 7);
    let script = |n: u32| format!("a = {a}\nl = Sequence(a * 3, k, 1, {n})");
    let (c, work) = work_of(|| load(&script(12)).unwrap());
    assert!(is_defined(&c, "l"));
    assert!(
        (1_200_000..2_000_000).contains(&work),
        "{work} units of work"
    );
    let (c, work) = work_of(|| load(&script(2000)).unwrap());
    assert!(!is_defined(&c, "l"));
    assert!(work <= PAST_WORK_BOUND, "{work} units of work");
    // 3 000 dense digits: a product of 9 million digit products, a quotient
    // and a power, each with the value of the unbounded evaluation
    let b = dense_digits(3000, 8);
    for (body, most) in [
        ("b * b", 1_200_000),
        ("1 / b", 1_400_000),
        ("b^365", 900_000),
    ] {
        let script = format!("b = {b}\nl = Sequence({body}, k, 1, 3)");
        let (c, work) = work_of(|| load(&script).unwrap());
        assert!(is_defined(&c, "l"), "{body}");
        assert!(work <= most, "{body}: {work} units of work");
        let free = crate::expr::eval::unbounded(|| load(&script).unwrap());
        assert_eq!(value_of_l(&c), value_of_l(&free), "{body}");
    }
}

/// Small lists and the exact work each does, so that a charge that goes
/// missing (or changes) is seen: the counter does not depend on the
/// machine. Each line shows one kind of charge. `Sequence(1.5 + 2.25, k, 1,
/// 4)`, for one: the bounds (two visits of 3), counting 4 values, and for
/// each value a step of filling, three visits, the sum (3) and its double
/// (90): `6 + 4 + 4 * 103 = 422`.
const EXACT_WORK: &[(&str, &str, u64)] = &[
    ("a range, its values", "l = Sequence(k, k, 1, 4)", 26),
    (
        "full visits, a power, a function",
        "l = Sequence(k^2 + sin(k), k, 1, 4)",
        134,
    ),
    (
        "a sampled function",
        "f(x) = 2x + 1\nl = Sequence(f(k), k, 1, 4)",
        58,
    ),
    (
        "the branch of `If` taken",
        "l = Sequence(If(k < 2, k, k + k + k), k, 1, 4)",
        110,
    ),
    (
        "a sum of short decimals",
        "l = Sequence(1.5 + 2.25, k, 1, 4)",
        422,
    ),
    ("a short quotient", "l = Sequence(1 / 3, k, 1, 4)", 810),
    (
        "a 41-digit product",
        "l = Sequence(1.2345678901234567890123456789012345678901 * 3, k, 1, 4)",
        890,
    ),
    (
        "a rounding",
        "l = Sequence(round(1.23456, 2), k, 1, 4)",
        422,
    ),
    (
        "the decimal of a double",
        "l = Sequence(floor(2.5) * 0.5, k, 1, 4)",
        610,
    ),
    ("a short power", "l = Sequence(1.5^3, k, 1, 4)", 562),
    ("a long power", "l = Sequence(1.0001^365, k, 1, 4)", 10070),
    (
        "an 81-digit product",
        "l = Sequence((1 + 1E-80) * 3, k, 1, 4)",
        2658,
    ),
    (
        "an 81-digit quotient",
        "l = Sequence(1 / (1 + 1E-80), k, 1, 4)",
        9694,
    ),
    (
        "a list object read",
        "L = Sequence(5)\nl = Sequence(Max(L) + Length(L) + L(k), k, 1, 4)",
        114,
    ),
    ("a sort", "l = Sort(Sequence(5 - k, k, 1, 4))", 66),
];

#[test]
fn small_lists_do_exactly_their_work() {
    for &(what, script, units) in EXACT_WORK {
        let (c, work) = work_of(|| load(script).unwrap());
        assert!(is_defined(&c, "l"), "{what}");
        assert_eq!(work, units, "{what}");
    }
}

// The commands below are ported from GeoGebra's own tests where it has them
// (`CommandsTest.cmdClasses`, `cmdFlatten`, `cmdFrequency`, `cmdIndexOf`,
// `cmdInsert`, `cmdIntersection`, `cmdOrdinalRank`, `cmdPointList`,
// `cmdRemoveUndefined`, `cmdSort`, `cmdTake`, `cmdTiedRank`, `cmdUnion`,
// `cmdUnique`, `cmdZip`) and from the upstream algorithms otherwise
// (`AlgoTake`, `AlgoRemoveUndefined`, `AlgoFlatten`, `AlgoInsert`,
// `AlgoUnique`, `AlgoFrequency`, `AlgoClasses`, `AlgoZip`, `AlgoUnion`,
// `AlgoIntersection`, `AlgoRemove`, `AlgoPointList`, `AlgoOrdinalRank`,
// `AlgoTiedRank`), with the degenerate inputs of the harness cases.

#[test]
fn take_reads_a_run_of_the_list() {
    let mut c = load(
        "n = 1\n\
         m = 3\n\
         u = 0 / 0\n\
         l = {1, 2, 3, 4, 5}\n\
         a = Take(l, 1, 3)\n\
         b = Take(l, 4)\n\
         d = Take(l, n, m)\n\
         e = Take({}, 1, 2)\n\
         f = Take(l, 0)\n\
         g = Take(l, 2, 6)\n\
         h = Take(l, 3, 2)\n\
         i = Take(l, 2, 0)\n\
         j = Take(l, 9, 0)\n\
         k = Take(l, 1.9, 3.7)\n\
         t = Take(l, u, 3)\n\
         w = Take(l, 1, u)\n\
         p = Take({(1, 1), (2, 2), (3, 3)}, 2, 3)",
    )
    .unwrap();
    assert_eq!(elems(&c, "a"), "{1, 2, 3}");
    assert_eq!(elems(&c, "b"), "{4, 5}");
    assert_eq!(elems(&c, "d"), "{1, 2, 3}");
    // an empty or out-of-range run is undefined; a zero end inside the list
    // is an empty run (`AlgoTake`)
    assert_eq!(elems(&c, "e"), "undefined");
    assert_eq!(elems(&c, "f"), "undefined");
    assert_eq!(elems(&c, "g"), "undefined");
    assert_eq!(elems(&c, "h"), "undefined");
    assert_eq!(elems(&c, "i"), "{}");
    assert_eq!(elems(&c, "j"), "undefined");
    // the bounds are truncated towards zero
    assert_eq!(elems(&c, "k"), "{1, 2, 3}");
    assert_eq!(elems(&c, "p"), "{(2, 2), (3, 3)}");
    // an undefined bound gives an empty list
    assert_eq!(elems(&c, "t"), "{}");
    assert_eq!(elems(&c, "w"), "{}");
    // `a` keeps its value below
    assert_eq!(elems(&c, "a"), "{1, 2, 3}");
    // a run that grows past the end appears and disappears again
    c.set_number(c.id("n").unwrap(), 3.0).unwrap();
    c.set_number(c.id("m").unwrap(), 6.0).unwrap();
    assert_eq!(elems(&c, "d"), "undefined");
    c.set_number(c.id("m").unwrap(), 5.0).unwrap();
    assert_eq!(elems(&c, "d"), "{3, 4, 5}");
    rebuilt(&c);
}

#[test]
fn remove_undefined_keeps_the_defined_elements() {
    let c = load(
        "l = {1, 1 / 0, 2, sqrt(-1), 3}\n\
         a = RemoveUndefined(l)\n\
         b = RemoveUndefined({})\n\
         d = RemoveUndefined({{1, 1 / 0}, {2, 3}})\n\
         e = RemoveUndefined({true, 1 / 0})\n\
         f = RemoveUndefined({(1, 1), (1 / 0, 2)})",
    )
    .unwrap();
    assert_eq!(elems(&c, "a"), "{1, 2, 3}");
    assert_eq!(elems(&c, "b"), "{}");
    // a defined list with undefined elements is itself defined
    assert_eq!(elems(&c, "d"), "{{1, ?number}, {2, 3}}");
    assert_eq!(elems(&c, "e"), "{true}");
    assert_eq!(elems(&c, "f"), "{(1, 1)}");
    rebuilt(&c);
}

#[test]
fn flatten_removes_every_level_of_nesting() {
    let c = load(
        "a = Flatten({})\n\
         b = Flatten({{}, {{{}}}})\n\
         d = Flatten({{(1, 1)}, {{{4}}}})\n\
         e = Flatten({1, {2, {3}}})\n\
         f = Flatten({1, 2})",
    )
    .unwrap();
    // `CommandsTest.cmdFlatten`
    assert_eq!(elems(&c, "a"), "{}");
    assert_eq!(elems(&c, "b"), "{}");
    assert_eq!(elems(&c, "d"), "{(1, 1), 4}");
    assert_eq!(elems(&c, "e"), "{1, 2, 3}");
    assert_eq!(elems(&c, "f"), "{1, 2}");
    rebuilt(&c);
}

#[test]
fn unique_sorts_numbers_and_dedups_the_rest_in_order() {
    let c = load(
        "a = Unique({1, 2, 3, 3, 4, 5})\n\
         b = Unique({})\n\
         d = Unique({5, 1, 4, 1})\n\
         e = Unique({1 / 3, 1 / 3 + 1E-16})\n\
         f = Unique({(2, 1), (1, 1), (2, 1)})\n\
         g = Unique({true, true, false})",
    )
    .unwrap();
    // `CommandsTest.cmdUnique`: numbers ascending, values within `1e-8` of
    // each other are one (`FrequencyGgb`)
    assert_eq!(elems(&c, "a"), "{1, 2, 3, 4, 5}");
    assert_eq!(elems(&c, "b"), "undefined");
    assert_eq!(elems(&c, "d"), "{1, 4, 5}");
    assert_eq!(elems(&c, "e"), "{0.3333333333333333}");
    // other kinds keep their first appearance (`AlgoUnion.addToOutputList`)
    assert_eq!(elems(&c, "f"), "{(2, 1), (1, 1)}");
    assert_eq!(elems(&c, "g"), "{true, false}");
    rebuilt(&c);
}

#[test]
fn frequency_counts_values_and_classes() {
    let c = load(
        "a = Frequency({2, 3, 4})\n\
         b = Frequency({1, 2, 3, 3, 4, 5})\n\
         d = Frequency(true, {1, 2, 3, 3, 4, 5})\n\
         e = Frequency(false, {1, 2, 3, 4, 5}, {2, 3, 4})\n\
         f = Frequency(false, {1, 2, 3, 4, 5}, {2, 3, 4}, true, 4)\n\
         g = Frequency({1, 2, 3, 4, 5}, {2, 3, 4}, false)\n\
         h = Frequency({1, 2, 3, 4, 5}, {2, 3, 4}, true, 4)\n\
         i = Frequency({2, 3, 4})",
    )
    .unwrap();
    // `CommandsTest.cmdFrequency`
    assert_eq!(elems(&c, "a"), "{1, 1, 1}");
    assert_eq!(elems(&c, "b"), "{1, 1, 2, 1, 1}");
    assert_eq!(elems(&c, "d"), "{1, 2, 4, 5, 6}");
    assert_eq!(elems(&c, "e"), "{0, 1, 1, 1}");
    assert_eq!(elems(&c, "f"), "{0, 4, 4, 4}");
    assert_eq!(elems(&c, "g"), "{0, 1, 1, 1}");
    assert_eq!(elems(&c, "h"), "{0, 4, 4, 4}");
    assert_eq!(elems(&c, "i"), "{1, 1, 1}");
    rebuilt(&c);
}

#[test]
fn frequency_classes_edge_cases() {
    let c = load(
        "b = false\n\
         t = true\n\
         s = 4\n\
         l = {1, 2, 3, 4, 5}\n\
         m = {2, 3, 9}\n\
         a = Frequency(l, m)\n\
         d = Frequency({5, 4, 3}, {3, 4, 5})\n\
         e = Frequency(b, l, l)\n\
         f = Frequency({}, {1, 2})\n\
         g = Frequency(l, {})\n\
         h = Frequency({1}, {1, 2})\n\
         i = Frequency({(1, 1)}, {1, 2})\n\
         j = Frequency(t, l, m, t, s)\n\
         p = Frequency(l, m, t, s)\n\
         q = Frequency(l, m, t, -1)",
    )
    .unwrap();
    // three bounds cut two classes; the last one holds both its bounds
    assert_eq!(elems(&c, "a"), "{0, 1, 1, 0}");
    // a falling class list keeps its highest class first
    assert_eq!(elems(&c, "d"), "{2, 1}");
    assert_eq!(elems(&c, "e"), "{1, 1, 1, 2}");
    // empty data, an empty class list, a class list with one bound or a
    // non-numeric one is undefined
    assert_eq!(elems(&c, "f"), "undefined");
    assert_eq!(elems(&c, "g"), "undefined");
    assert_eq!(elems(&c, "h"), "undefined");
    assert_eq!(elems(&c, "i"), "undefined");
    // cumulative with density and a scale
    assert_eq!(elems(&c, "j"), "{0, 4, 8, 8}");
    assert_eq!(elems(&c, "p"), "{0, 4, 4, 0}");
    // a negative scale is undefined
    assert_eq!(elems(&c, "q"), "undefined");
    rebuilt(&c);
}

#[test]
fn classes_cut_the_range() {
    let c = load(
        "a = Classes({2, 3, 4}, 42)\n\
         b = Classes({2, 3, 4}, 13, 4)\n\
         d = Classes({2, 3, 4}, 2)\n\
         e = Classes({2, 3, 4}, 4.9)\n\
         f = Classes({2, 3, 4}, 0)\n\
         g = Classes({2, 3, 4}, 3, 0.5)\n\
         h = Classes({2, 3, 4}, 3, -1)\n\
         i = Classes({}, 3)\n\
         j = Classes({2, 3, 4}, 13, 4)",
    )
    .unwrap();
    // `CommandsTest.cmdClasses`: 43 cuts from 2 to 4
    let a = c.id("a").unwrap();
    assert_eq!(c.elements(a).unwrap().len(), 43);
    let first = match c.elements(a).unwrap().get(0) {
        Some(crate::expr::Element::Value(_, Value::Number(v))) => v,
        other => panic!("first class {other:?}"),
    };
    assert!((first - 2.0).abs() < 1e-12);
    // a start past the data gives one class only, which is undefined
    assert_eq!(elems(&c, "b"), "undefined");
    assert_eq!(elems(&c, "j"), "undefined");
    assert_eq!(elems(&c, "d"), "{2, 3, 4}");
    // the count is truncated towards zero
    assert_eq!(elems(&c, "e"), "{2, 2.5, 3, 3.5, 4}");
    // a count below 1 is undefined
    assert_eq!(elems(&c, "f"), "undefined");
    assert_eq!(elems(&c, "g"), "{3, 3.5, 4}");
    // a width of 0 or less is undefined
    assert_eq!(elems(&c, "h"), "undefined");
    assert_eq!(elems(&c, "i"), "undefined");
    rebuilt(&c);
}

#[test]
fn index_of_finds_the_first_hit() {
    let mut c = load(
        "k = 4\n\
         l = {1, 3, 5, 2, 5, 4}\n\
         a = IndexOf(5, l)\n\
         b = IndexOf(5, l, k)\n\
         d = IndexOf(5, l, 6)\n\
         e = IndexOf(5, l, 0)\n\
         f = IndexOf(5, l, 2.9)\n\
         g = IndexOf((1, 2), {(0, 0), (1, 2)})\n\
         h = IndexOf(3, {1, 2})\n\
         i = IndexOf(true, {false, true})\n\
         j = IndexOf(5, l, 0 / 0)",
    )
    .unwrap();
    // `CommandsTest.cmdIndexOf`
    assert_eq!(num(&c, "a"), 3.0);
    assert_eq!(num(&c, "b"), 5.0);
    assert_eq!(c.value(c.id("d").unwrap()), Value::Undefined);
    // a start below 1 is undefined
    assert_eq!(c.value(c.id("e").unwrap()), Value::Undefined);
    // the start is truncated towards zero, then counts from 1
    assert_eq!(num(&c, "f"), 3.0);
    assert_eq!(num(&c, "g"), 2.0);
    assert_eq!(c.value(c.id("h").unwrap()), Value::Undefined);
    assert_eq!(num(&c, "i"), 2.0);
    // the search starts inside the list when the start grows
    c.set_number(c.id("k").unwrap(), 5.0).unwrap();
    assert_eq!(num(&c, "b"), 5.0);
    c.set_number(c.id("k").unwrap(), 6.0).unwrap();
    assert_eq!(c.value(c.id("b").unwrap()), Value::Undefined);
    // an undefined start is undefined
    assert_eq!(c.value(c.id("j").unwrap()), Value::Undefined);
    rebuilt(&c);
}

#[test]
fn insert_splices_at_a_position() {
    let c = load(
        "l = {1, 2, 3, 4, 5}\n\
         m = {1, 2}\n\
         a = Insert(l, l, 4)\n\
         b = Insert(9, m, 2)\n\
         d = Insert(9, m, -1)\n\
         e = Insert(9, m, -2)\n\
         f = Insert(9, m, 0)\n\
         g = Insert(9, m, 4)\n\
         h = Insert(9, m, 1.9)\n\
         i = Insert(9, {}, 1)\n\
         j = Insert({}, m, 1)\n\
         k = Insert({3, 4}, m, 2)",
    )
    .unwrap();
    // `CommandsTest.cmdInsert`: a list inserts its elements
    assert_eq!(elems(&c, "a"), "{1, 2, 3, 1, 2, 3, 4, 5, 4, 5}");
    assert_eq!(elems(&c, "b"), "{1, 9, 2}");
    // -1 is after the last element, -2 before it
    assert_eq!(elems(&c, "d"), "{1, 2, 9}");
    assert_eq!(elems(&c, "e"), "{1, 9, 2}");
    assert_eq!(elems(&c, "f"), "undefined");
    assert_eq!(elems(&c, "g"), "undefined");
    // the position is truncated towards zero
    assert_eq!(elems(&c, "h"), "{9, 1, 2}");
    assert_eq!(elems(&c, "i"), "{9}");
    assert_eq!(elems(&c, "j"), "{1, 2}");
    assert_eq!(elems(&c, "k"), "{1, 3, 4, 2}");
    rebuilt(&c);
}

#[test]
fn intersection_union_and_remove_dedup_by_equality() {
    let c = load(
        "a = Intersection({1, 2, 3, 4, 5}, {1, 2, 3, 4, 5})\n\
         b = Intersection({3, 1}, {1, 2, 3, 3})\n\
         d = Intersection({1, 2}, {})\n\
         e = Union({1, 2, 3}, {2, 2, 2, 4, 4, 4})\n\
         f = Union({1, 2}, {})\n\
         g = Remove({1, 2, 3, 4, 5}, {2, 4})\n\
         h = Remove({1, 2, 3}, {9})\n\
         i = Remove({1, 1 / 0, 2}, {1 / 0, 1})\n\
         j = Remove({1, 2}, {})",
    )
    .unwrap();
    // `CommandsTest.cmdIntersection`, `cmdUnion`
    assert_eq!(elems(&c, "a"), "{1, 2, 3, 4, 5}");
    // the second list gives the order of the shared elements
    assert_eq!(elems(&c, "b"), "{1, 3}");
    assert_eq!(elems(&c, "d"), "{}");
    assert_eq!(elems(&c, "e"), "{1, 2, 3, 4}");
    assert_eq!(elems(&c, "f"), "{1, 2}");
    assert_eq!(elems(&c, "g"), "{1, 3, 5}");
    assert_eq!(elems(&c, "h"), "{1, 2, 3}");
    // an undefined element removes one undefined element
    assert_eq!(elems(&c, "i"), "{2}");
    assert_eq!(elems(&c, "j"), "{1, 2}");
    rebuilt(&c);
}

#[test]
fn sort_permutes_a_value_list_by_its_keys_and_wraps_points() {
    let c = load(
        "A = (3, 1)\n\
         B = (1, 2)\n\
         C = (3, 0)\n\
         a = Sort({3, 1, 2}, {10, 20, 30})\n\
         b = Sort({10, 20, 30}, {3, 1, 2})\n\
         k = {2, 1, 2}\n\
         c = Sort({10, 20, 30}, k)\n\
         d = Sort({1, 2, 3, 4, 5}, {2, 3, 4})\n\
         e = Sort(A, B, C)\n\
         f = Sort({(3, 1), (1, 2)})\n\
         g = Sort({3, 1, 2}, {A, (0, 0), (5, 5)})",
    )
    .unwrap();
    // the second list holds the keys, and the values of the first one
    // follow them (`AlgoSort(cons, label, valueList, inputList)`)
    assert_eq!(elems(&c, "a"), "{3, 1, 2}");
    assert_eq!(elems(&c, "b"), "{20, 30, 10}");
    // equal keys keep their order: the comparator never returns 0
    assert_eq!(elems(&c, "c"), "{20, 10, 30}");
    // a value list of the wrong size is undefined (`AlgoSort`)
    assert_eq!(elems(&c, "d"), "undefined");
    assert_eq!(elems(&c, "e"), "{(1, 2), (3, 0), (3, 1)}");
    assert_eq!(elems(&c, "f"), "{(1, 2), (3, 1)}");
    // point keys sort by x (`GeoPoint.getComparatorX`)
    assert_eq!(elems(&c, "g"), "{1, 3, 2}");
    rebuilt(&c);
}

#[test]
fn point_list_ranks_and_literal_arguments() {
    let c = load(
        "a = PointList({{1, 2}, {3, 4}})\n\
         b = PointList({1, 2, 3, 4, 5})\n\
         d = PointList({{1, 2}, {3}, {1, 2}})\n\
         e = OrdinalRank({1, 2, 3, 4, 5})\n\
         f = OrdinalRank({3, 1, 3})\n\
         g = OrdinalRank({})\n\
         h = OrdinalRank(3, 1, 2)\n\
         i = TiedRank({1, 2, 3, 4, 5})\n\
         j = TiedRank({3, 1, 3})\n\
         k = TiedRank({})\n\
         l = TiedRank(3, 1, 3)\n\
         m = Reverse(1, 2, 3)\n\
         n = Reverse((3, 1), (1, 2))\n\
         p = PointList(1, 2)\n\
         q = PointList({})\n\
         r = OrdinalRank({(1, 2), (0, 0)})\n\
         s = TiedRank({true, false})",
    )
    .unwrap();
    // `CommandsTest.cmdPointList`: numbers alone are no point
    assert_eq!(elems(&c, "a"), "{(1, 2), (3, 4)}");
    assert_eq!(elems(&c, "b"), "{}");
    assert_eq!(elems(&c, "d"), "{(1, 2), (1, 2)}");
    // an empty input makes an undefined list (`AlgoPointList`)
    assert_eq!(elems(&c, "q"), "undefined");
    // elements that are no numbers make an undefined list, but the line
    // still runs (`AlgoOrdinalRank`, `AlgoTiedRank`)
    assert_eq!(elems(&c, "r"), "undefined");
    assert_eq!(elems(&c, "s"), "undefined");
    // `CommandsTest.cmdOrdinalRank`, `cmdTiedRank`
    assert_eq!(elems(&c, "e"), "{1, 2, 3, 4, 5}");
    assert_eq!(elems(&c, "f"), "{2, 1, 3}");
    assert_eq!(elems(&c, "g"), "undefined");
    assert_eq!(elems(&c, "h"), "{3, 1, 2}");
    assert_eq!(elems(&c, "i"), "{1, 2, 3, 4, 5}");
    // tied values share the average of their ranks
    assert_eq!(elems(&c, "j"), "{2.5, 1, 2.5}");
    assert_eq!(elems(&c, "k"), "undefined");
    assert_eq!(elems(&c, "l"), "{2.5, 1, 2.5}");
    assert_eq!(elems(&c, "m"), "{3, 2, 1}");
    assert_eq!(elems(&c, "n"), "{(1, 2), (3, 1)}");
    assert_eq!(elems(&c, "p"), "{}");
    rebuilt(&c);
}

#[test]
fn zip_walks_its_lists_together() {
    let c = load(
        "a = Zip(t^2, t, {1, 2, 3, 4, 5})\n\
         b = Zip(i, i, {1, 2})\n\
         d = Zip(i + j, i, {1, 2}, j, {3, 4})\n\
         e = Zip(i + j, j, {1, 2}, i, {3, 4})\n\
         f = Zip(2 * A, A, {(1, 1), (2, 3)})\n\
         g = Zip(k, k, {1, 2}, m, {3})\n\
         u = (0, 1)\n\
         w = (0, 2)\n\
         h = Zip(A + B, A, {(1, 0), (2, 0)}, B, {u, w})\n\
         i = Zip(t^2, t, Sequence(4))\n\
         j = Zip({t, t + 1}, t, {1, 2})",
    )
    .unwrap();
    // `CommandsTest.cmdZip`
    assert_eq!(elems(&c, "a"), "{1, 4, 9, 16, 25}");
    assert_eq!(elems(&c, "b"), "{1, 2}");
    assert_eq!(elems(&c, "d"), "{4, 6}");
    assert_eq!(elems(&c, "e"), "{4, 6}");
    assert_eq!(elems(&c, "f"), "{(2, 2), (4, 6)}");
    // the shortest list ends the walk
    assert_eq!(elems(&c, "g"), "{1}");
    assert_eq!(elems(&c, "h"), "{(1, 1), (2, 2)}");
    assert_eq!(elems(&c, "i"), "{1, 4, 9, 16}");
    assert_eq!(elems(&c, "j"), "{{1, 2}, {2, 3}}");
    rebuilt(&c);
}

#[test]
fn product_reads_frequencies_and_an_expression() {
    let c = load(
        "a = Product({2, 3}, {3, 2})\n\
         b = Product({2, 3, 4}, {1, 2})\n\
         d = Product({2, 3, 4}, {-1})\n\
         e = Product(k^2, k, 1, 4)\n\
         f = Product(k^2, k, 3, 1)\n\
         g = Product(k^2, k, 1.5, 3.8)\n\
         h = Product(k, k, 1, 0)",
    )
    .unwrap();
    // a list of frequencies repeats each value (`AlgoStats1D`)
    assert_eq!(num(&c, "a"), 72.0);
    // a class list one longer than the frequencies uses midpoints
    assert_eq!(num(&c, "b"), 2.5 * 3.5 * 3.5);
    // a negative frequency is undefined
    assert_eq!(c.value(c.id("d").unwrap()), Value::Undefined);
    // `AlgoFoldExpression`: the expression runs over whole numbers
    assert_eq!(num(&c, "e"), 576.0);
    // the bounds are truncated towards zero; an empty fold is the value at
    // the start
    assert_eq!(num(&c, "f"), 9.0);
    assert_eq!(num(&c, "g"), 2.25 * 4.0 * 9.0);
    assert_eq!(num(&c, "h"), 1.0);
    rebuilt(&c);
}

#[test]
fn element_walks_into_nested_lists() {
    let mut c = load(
        "k = 1\n\
         m = {{1, 2}, {3, 4}}\n\
         a = Element(m, 1, 2)\n\
         b = Element(m, 2, 1)\n\
         d = Element(m, 1, 3)\n\
         e = Element(m, 2)",
    )
    .unwrap();
    assert_eq!(num(&c, "a"), 2.0);
    assert_eq!(num(&c, "b"), 3.0);
    assert_eq!(c.value(c.id("d").unwrap()), Value::Undefined);
    assert_eq!(elems(&c, "e"), "{3, 4}");
    c.set_number(c.id("k").unwrap(), 2.0).unwrap();
    rebuilt(&c);
}

#[test]
fn first_and_last_read_a_locus() {
    let c = load(
        "A = (0, 0)\n\
         B = (2, 0)\n\
         s = Segment(A, B)\n\
         P = Point(s)\n\
         Q = P + (1, 1)\n\
         l = Locus(Q, P)\n\
         a = First(l, 2)\n\
         b = First(l, 1)",
    )
    .unwrap();
    // `AlgoFirstLocus`: the first n points of the locus
    let a = c.id("a").unwrap();
    assert_eq!(c.elements(a).unwrap().len(), 2);
    let b = c.id("b").unwrap();
    assert_eq!(c.elements(b).unwrap().len(), 1);
    rebuilt(&c);
}
