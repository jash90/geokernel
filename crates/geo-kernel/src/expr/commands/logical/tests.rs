use crate::expr::tests::{num, rebuilt, sample, truth};
use crate::kernel::{Construction, Kind, Value};
use crate::script::{exec, load};

#[test]
fn comparisons_use_the_tolerance_of_doubles() {
    let mut c = load(
        "a = 2\n\
         b = a < 3\n\
         d = a <= 3\n\
         g = a == 3\n\
         h = a != 3\n\
         k = a > 3\n\
         m = a \u{2265} 3\n\
         n = 0.1 + 0.2 == 0.3\n\
         p = 1 < 2",
    )
    .unwrap();
    assert_eq!(c.kind(c.id("b").unwrap()), Kind::Boolean);
    let a = c.id("a").unwrap();
    let all = |c: &Construction| ["b", "d", "g", "h", "k", "m"].map(|l| truth(c, l));
    assert_eq!(all(&c), [true, true, false, true, false, false]);
    // `isGreater(b, a)` is `b > a + 1e-8`: within 1e-8 of 3, `a < 3` is
    // false and `a <= 3`, `a >= 3` and `a == 3` are true
    for v in [3.0, 3.0 - 1e-9, 3.0 + 1e-9] {
        c.set_number(a, v).unwrap();
        assert_eq!(all(&c), [false, true, true, false, false, true], "{v}");
    }
    c.set_number(a, 3.0 - 1e-7).unwrap();
    assert_eq!(all(&c), [true, true, false, true, false, false]);
    c.set_number(a, 3.0 + 1e-7).unwrap();
    assert_eq!(all(&c), [false, false, false, true, true, true]);
    // decided on doubles, not on the exact decimals
    assert!(truth(&c, "n") && truth(&c, "p"));
    // a constant boolean is an expression that never changes
    assert!(c.set_number(c.id("p").unwrap(), 0.0).is_err());
    rebuilt(&c);
}

#[test]
fn an_undefined_operand_makes_a_comparison_false_never_undefined() {
    let mut c = load(
        "a = 1\n\
         d = 1 / a\n\
         r = sqrt(a)\n\
         b = d > 3\n\
         g = d < 3\n\
         h = r < 3\n\
         k = r != 3\n\
         m = r == r\n\
         n = !(r < 3)",
    )
    .unwrap();
    c.set_number(c.id("a").unwrap(), 0.0).unwrap();
    // `d` is infinite: compared as a value, `defined` is dropped by
    // `AlgoDependentBoolean`
    assert!(truth(&c, "b") && !truth(&c, "g"));
    c.set_number(c.id("a").unwrap(), -1.0).unwrap();
    assert_eq!(
        ["h", "k", "m", "n"].map(|l| truth(&c, l)),
        [false, true, false, true]
    );
    c.set_number(c.id("a").unwrap(), 4.0).unwrap();
    assert_eq!(
        ["h", "k", "m", "n"].map(|l| truth(&c, l)),
        [true, true, true, false]
    );
    rebuilt(&c);
}

#[test]
fn logical_operators_follow_the_grammar() {
    let c = load(
        "a = 2\n\
         A = (1, 2)\n\
         B = (1, 2)\n\
         C = (3, 3)\n\
         u = (1, 2)\n\
         t = true\n\
         f = False\n\
         b = 1 < a < 3\n\
         d = 1 < a && a < 3\n\
         g = a < 3 || a > 5 && a > 7\n\
         h = t \u{2295} f\n\
         k = t -> f\n\
         m = f \u{2192} t \u{2192} f\n\
         n = !t == f\n\
         p = \u{ac}(a < 3) \u{2228} t \u{2227} f\n\
         q = A == B\n\
         r = A == u\n\
         s = A == (1, 2 + 1E-9)\n\
         v = A != C\n\
         w = A == a\n\
         zz = (a < 3) + 1\n\
         yy = true > false\n\
         o = AreCollinear(A, B, C) && t\n\
         j = 3 < a < 1 == f",
    )
    .unwrap();
    for (label, want) in [
        ("b", true),
        ("d", true),
        // `&&` binds tighter than `||`
        ("g", true),
        ("h", true),
        ("k", false),
        // left associative: `(f -> t) -> f`
        ("m", false),
        // `!` binds tightest: `(!t) == f`
        ("n", true),
        ("p", false),
        ("q", true),
        ("r", true),
        ("s", true),
        ("v", true),
        // a number and a point are never equal
        ("w", false),
        ("yy", true),
        ("o", true),
        // `3 < a && a < 1 && 1 == f`
        ("j", false),
    ] {
        assert_eq!(truth(&c, label), want, "{label}");
    }
    assert_eq!(num(&c, "zz"), 2.0);
    rebuilt(&c);
}

#[test]
fn bars_split_where_the_preprocessor_splits_them() {
    let c =
        load("a = -3\nb = 2\nc = ||a| - |b||\nd = |a - |b||\nt = true\ng = t || a < 0").unwrap();
    assert_eq!(num(&c, "c"), 1.0);
    assert_eq!(num(&c, "d"), 5.0);
    assert!(truth(&c, "g"));
}

#[test]
fn misplaced_booleans_are_errors() {
    let base = "a = 2\nA = (1, 2)\nB = (3, 4)\nt = true";
    for line in [
        "b = a && t",
        "b = t || 1",
        "b = \u{ac}a",
        "b = !A",
        "b = A < B",
        "b = A >= 1",
        "f(x) = x < A",
        "f(x) = (x < 3) && a",
        "b = a = 3",
        "b = a < ",
        "b = == a",
    ] {
        let mut c = load(base).unwrap();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), 4, "`{line}` left an object behind");
    }
    for label in ["true", "False"] {
        assert!(exec(&mut load(base).unwrap(), &format!("{label} = 1")).is_err());
    }
}

#[test]
fn boolean_function_applications_can_be_used_as_conditions() {
    let c = load("f(x) = x < 3\nb = f(2)\nc = f(4)\nd = If(f(2), 1, 2)").unwrap();
    assert!(truth(&c, "b"));
    assert!(!truth(&c, "c"));
    assert_eq!(num(&c, "d"), 1.0);
}

#[test]
fn if_picks_the_first_true_condition() {
    let mut c = load(
        "a = 2\n\
         A = (1, 2)\n\
         r = sqrt(a)\n\
         b = If(a < 3, 1, 2)\n\
         d = If(a < 3, 5)\n\
         g = If(a < 1, 10, a < 3, 20, 30)\n\
         h = If(a < 1, 10, a < 3, 20)\n\
         P = If(a < 3, A, 2)\n\
         Q = If(a < 3, 2, A)\n\
         t = If(a < 3, a > 0)\n\
         k = If(r < 3, 1, 2)\n\
         m = 2 If(a < 3, a, -a) + 1\n\
         n = 0.2 + If(a < 3, 0.1, 0.2)\n\
         o = If(a < 3, 0.1, 0.2) + 0.2\n\
         p = If(a < 3, 1 / 0, 2)",
    )
    .unwrap();
    let a = c.id("a").unwrap();
    assert_eq!(c.kind(c.id("P").unwrap()), Kind::Point);
    assert_eq!(c.kind(c.id("t").unwrap()), Kind::Boolean);
    let check = |c: &Construction, want: [Option<f64>; 4]| {
        for (label, w) in ["b", "d", "g", "h"].into_iter().zip(want) {
            match w {
                Some(v) => assert_eq!(num(c, label), v, "{label}"),
                None => assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined, "{label}"),
            }
        }
    };
    check(&c, [Some(1.0), Some(5.0), Some(20.0), Some(20.0)]);
    assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 1.0, y: 2.0 });
    // a number in a point result is the point `(v, 0)`
    assert_eq!(c.value(c.id("Q").unwrap()), Value::Point { x: 2.0, y: 0.0 });
    assert!(truth(&c, "t"));
    assert_eq!(num(&c, "k"), 1.0);
    assert_eq!(num(&c, "m"), 5.0);
    // the branch's exact decimal goes with it (`GeoNumeric.set`), and a
    // constant branch keeps its class (`reuseDefinition`): `0.1 + 0.2` is
    // 0.3 on either side
    assert_eq!(num(&c, "n"), 0.3);
    assert_eq!(num(&c, "o"), 0.3);
    assert!(c.infinite_number(c.id("p").unwrap()).is_some());
    c.set_number(a, 0.5).unwrap();
    check(&c, [Some(1.0), Some(5.0), Some(10.0), Some(10.0)]);
    c.set_number(a, 4.0).unwrap();
    check(&c, [Some(2.0), None, Some(30.0), None]);
    assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 2.0, y: 0.0 });
    assert!(!truth(&c, "t"));
    assert_eq!(num(&c, "m"), -7.0);
    assert_eq!(num(&c, "n"), 0.4);
    // an undefined condition is false
    c.set_number(a, -1.0).unwrap();
    assert_eq!(num(&c, "k"), 2.0);
    rebuilt(&c);
    c.set_number(a, 2.0).unwrap();
    check(&c, [Some(1.0), Some(5.0), Some(20.0), Some(20.0)]);
}

#[test]
fn a_conditional_function_is_evaluated_at_each_x() {
    let mut c = load(
        "a = 1\n\
         r = sqrt(a)\n\
         f(x) = If(x < 0, -x, x)\n\
         g(x) = 2 f(x) + 1\n\
         h(x) = If(x < 0, x^2)\n\
         k(x) = If(x < -1, 1, x < 1, 2, 3)\n\
         m(x) = If(x < r, 1, 2)\n\
         n(x) = If(x < 0, a, 2)\n\
         p(x) = a + If(x < 0, 1, 2)\n\
         q(x) = If(x < -1, 1, x < 1, 2)\n\
         v = f(-3)\n\
         w = k(0.5) + f(a - 3)",
    )
    .unwrap();
    assert_eq!(sample(&c, "f", -2.0), Some(2.0));
    assert_eq!(sample(&c, "g", -2.0), Some(5.0));
    assert_eq!(sample(&c, "h", -2.0), Some(4.0));
    assert!(sample(&c, "h", 1.0).is_some_and(f64::is_nan));
    assert_eq!(
        [-2.0, 0.0, 2.0].map(|x| sample(&c, "k", x)),
        [Some(1.0), Some(2.0), Some(3.0)]
    );
    assert!(sample(&c, "q", 2.0).is_some_and(f64::is_nan));
    assert_eq!(num(&c, "v"), 3.0);
    assert_eq!(num(&c, "w"), 4.0);
    rebuilt(&c);
    let a = c.id("a").unwrap();
    c.set_number(a, -1.0).unwrap();
    // `r` is undefined: `x < r` reads an object, so it is a helper
    // function whose value is false; the function stays defined, as only
    // objects outside `If` count
    assert_eq!(sample(&c, "m", 0.0), Some(2.0));
    // `a` is defined (-1); make it undefined through `r`
    exec(&mut c, "s(x) = If(x < 0, r, 2)").unwrap();
    exec(&mut c, "u(x) = r + If(x < 0, 1, 2)").unwrap();
    assert_eq!(sample(&c, "s", 1.0), Some(2.0));
    assert!(sample(&c, "s", -1.0).is_some_and(f64::is_nan));
    assert_eq!(c.value(c.id("u").unwrap()), Value::Undefined);
    assert_eq!(sample(&c, "p", -1.0), Some(0.0));
    assert_eq!(sample(&c, "n", -1.0), Some(-1.0));
}

#[test]
fn bad_if_lines_are_errors() {
    let base = "a = 2\nA = (1, 2)\nt = true";
    for line in [
        "b = If(a < 3)",
        "b = If(1, 2, 3)",
        "b = If(a, 2)",
        "b = If(t, 1, a, 2, 3)",
        "f(x) = If(x < 0, (1, 2), 3)",
        "f(x) = If(x < 0, x < 1, 3)",
        "b = If(a < 3, 1, 2",
    ] {
        let mut c = load(base).unwrap();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), 3, "`{line}` left an object behind");
    }
    // a constant `If` is a command: dependent, it cannot be set
    let mut c = load("b = If(1 < 2, 3, 4)").unwrap();
    assert_eq!(num(&c, "b"), 3.0);
    assert!(c.set_number(c.id("b").unwrap(), 2.0).is_err());
}
