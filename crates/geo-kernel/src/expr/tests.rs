use crate::kernel::{Construction, Def, Kind, Value};
use crate::script::{exec, load};

fn num(c: &Construction, label: &str) -> f64 {
    match c.value(c.id(label).unwrap()) {
        Value::Number(v) => v,
        other => panic!("`{label}` is {other:?}"),
    }
}

#[test]
fn precedence_follows_the_grammar() {
    let c = load(
        "a = 3\n\
         c = -2^2\n\
         d = 2^-1\n\
         f = 2^3^2\n\
         g = -a^2\n\
         h = 2^-a^2\n\
         k = a/2a\n\
         m = a/2 a\n\
         n = 1/2/2\n\
         p = 3!^2\n\
         q = -3!\n\
         r = 2^3!\n\
         s = 2a!\n\
         t = a 3^2\n\
         u = 2(a + 1)\n\
         v = (a + 1)(a - 1)\n\
         w = 2^2\u{b2}\n",
    )
    .unwrap();
    for (label, want) in [
        ("c", -4.0),
        ("d", 0.5),
        ("f", 512.0),
        ("g", -9.0),
        ("h", 512.0),
        ("k", 4.5),
        ("m", 4.5),
        ("n", 0.25),
        ("p", 36.0),
        ("q", -6.0),
        ("r", 64.0),
        ("s", 12.0),
        ("t", 27.0),
        ("u", 8.0),
        ("v", 8.0),
        ("w", 16.0),
    ] {
        assert_eq!(num(&c, label), want, "{label}");
    }
}

#[test]
fn functions_constants_and_bars() {
    let c = load(
        "a = 2\n\
         b = 3\n\
         c = sin a\n\
         d = a sin b\n\
         f = |a - 5|\n\
         g = log(2, 8)\n\
         h = log(100)\n\
         k = ld(8)\n\
         m = atan2(1, 1)\n\
         n = round(2.675, 2)\n\
         p = e^0\n\
         q = 2pi\n",
    )
    .unwrap();
    assert_eq!(num(&c, "c"), 2f64.sin());
    assert_eq!(num(&c, "d"), 2.0 * 3f64.sin());
    assert_eq!(num(&c, "f"), 3.0);
    assert!((num(&c, "g") - 3.0).abs() < 1e-15);
    assert!((num(&c, "h") - 2.0).abs() < 1e-15);
    assert_eq!(num(&c, "k"), 3.0);
    assert_eq!(num(&c, "m"), std::f64::consts::FRAC_PI_4);
    assert_eq!(num(&c, "n"), 2.68);
    assert_eq!(num(&c, "p"), 1.0);
    assert_eq!(num(&c, "q"), std::f64::consts::TAU);
}

#[test]
fn degrees_make_floor_work_in_whole_degrees() {
    let c = load("a = 30.7\nf = a°\ng = floor(f)\nh = floor(2 f)\nk = floor(a)\n").unwrap();
    let deg = std::f64::consts::PI / 180.0;
    assert!((num(&c, "g") - 30.0 * deg).abs() < 1e-15);
    assert!((num(&c, "h") - 61.0 * deg).abs() < 1e-15);
    assert_eq!(num(&c, "k"), 30.0);
}

#[test]
fn infinity_is_kept_behind_an_undefined_number() {
    let mut c = load("a = 2\nc = 1/a\nd = 1/c\nf = atan(c)\n").unwrap();
    let a = c.id("a").unwrap();
    c.set_number(a, 0.0).unwrap();
    assert_eq!(c.value(c.id("c").unwrap()), Value::Undefined);
    assert_eq!(num(&c, "d"), 0.0);
    assert_eq!(num(&c, "f"), std::f64::consts::FRAC_PI_2);
    c.set_number(a, -0.0).unwrap();
    assert_eq!(num(&c, "f"), -std::f64::consts::FRAC_PI_2);
    assert!(num(&c, "d").is_sign_negative());
    // the same through a point that moves from 0 to -0
    let mut c = load("P = (0, 0)\nc = 1/x(P)\nf = atan(c)\n").unwrap();
    let p = c.id("P").unwrap();
    c.move_point(p, -0.0, 0.0).unwrap();
    assert_eq!(num(&c, "f"), -std::f64::consts::FRAC_PI_2);
}

#[test]
fn a_negative_base_with_a_fraction_has_a_real_root() {
    let c =
        load("a = -8\nc = a^(1/3)\nd = a^(2/3)\nf = a^(-1/3)\ng = a^-(1/3)\nh = a^0.5\n").unwrap();
    assert_eq!(num(&c, "c"), -2.0);
    assert!((num(&c, "d") - 4.0).abs() < 1e-15);
    assert_eq!(num(&c, "f"), -0.5);
    assert_eq!(num(&c, "g"), -0.5);
    assert_eq!(c.value(c.id("h").unwrap()), Value::Undefined);
}

#[test]
fn constant_expressions_are_free() {
    let mut c = load("c = 2 + 3\nf = c + 1\nP = (1, 2) * 2\nu = (1; 0)\n").unwrap();
    let id = c.id("c").unwrap();
    assert_eq!(c.def(id), Def::Number(5.0));
    c.set_number(id, 7.0).unwrap();
    assert_eq!(num(&c, "f"), 8.0);
    assert_eq!(c.def(c.id("P").unwrap()), Def::FreePoint { x: 2.0, y: 4.0 });
    assert_eq!(c.kind(c.id("u").unwrap()), Kind::Vector);
}

#[test]
fn points_and_vectors_follow_the_label() {
    let mut c = load(
        "A = (1, 2)\nB = (4, 6)\nu = (1, 2)\nv = (3, -1)\n\
         M = (A + B)/2\nm = (A + B)/2\ns = u v\nl = abs(v)\nS = A + 1\n\
         k = x(A) + y(B)\nd = Distance(A, B) / 2\nN = (Midpoint(A, B) + u) / 2\n",
    )
    .unwrap();
    let v = |c: &Construction, l: &str| c.value(c.id(l).unwrap());
    assert_eq!(v(&c, "M"), Value::Point { x: 2.5, y: 4.0 });
    assert_eq!(v(&c, "m"), Value::Vector { x: 2.5, y: 4.0 });
    assert_eq!(num(&c, "s"), 1.0);
    assert_eq!(num(&c, "l"), 10f64.sqrt());
    assert_eq!(v(&c, "S"), Value::Point { x: 2.0, y: 3.0 });
    assert_eq!(num(&c, "k"), 7.0);
    assert_eq!(num(&c, "d"), 2.5);
    assert_eq!(v(&c, "N"), Value::Point { x: 1.75, y: 3.0 });
    // no helper object for the commands
    assert_eq!(c.len(), 12);
    let a = c.id("A").unwrap();
    c.move_point(a, 0.0, 0.0).unwrap();
    assert_eq!(num(&c, "k"), 6.0);
    assert!((num(&c, "d") - 52f64.sqrt() / 2.0).abs() < 1e-15);
    assert_eq!(v(&c, "N"), Value::Point { x: 1.5, y: 2.5 });
}

#[test]
fn errors_leave_the_construction_unchanged() {
    let mut c = load("a = 1\nA = (0, 0)\n").unwrap();
    for bad in [
        "b = a +",
        "b = Z + 1",
        "b = (a, A)",
        "b = sin(A)",
        "b = ((((1",
        "b = 1 / Line(A, A)",
        "b = Distance(A, Midpoint(A, A))",
        "b = sin",
        "a = 2 + 2",
    ] {
        let before = (c.len(), c.exprs.len());
        assert!(exec(&mut c, bad).is_err(), "{bad}");
        assert_eq!((c.len(), c.exprs.len()), before, "{bad}");
    }
    let before = (c.len(), c.exprs.len());
    let deep = format!("b = {}1{}", "(".repeat(10_000), ")".repeat(10_000));
    assert!(exec(&mut c, &deep).is_err());
    let long = format!("b = 1{}", " + 1".repeat(100_000));
    assert!(exec(&mut c, &long).is_err());
    assert_eq!((c.len(), c.exprs.len()), before);
}

fn sample(c: &Construction, label: &str, x: f64) -> Option<f64> {
    c.function_value(c.id(label).unwrap(), x)
}

#[test]
fn functions_are_sampled_expanded_and_applied() {
    let mut c = load(
        "a = 2\n\
         f(x) = x^2 - 2*x + a\n\
         g(x) = f(x) + sin(x)\n\
         h(x) = f(f(x))\n\
         v = f(2)\n\
         w = h(a)\n\
         p = f(a) + g(1)\n\
         k(x) = 3\n",
    )
    .unwrap();
    assert_eq!(c.kind(c.id("f").unwrap()), Kind::Function);
    assert_eq!(c.value(c.id("k").unwrap()), Value::Function);
    assert_eq!(sample(&c, "f", -8.0), Some(82.0));
    assert_eq!(sample(&c, "g", 0.5), Some(1.729425538604203));
    assert_eq!(sample(&c, "h", -0.5), Some(6.0625));
    assert_eq!(sample(&c, "k", 1.0), Some(3.0));
    assert_eq!(sample(&c, "v", 1.0), None);
    assert_eq!(num(&c, "v"), 2.0);
    assert_eq!(num(&c, "w"), 2.0);
    assert_eq!(num(&c, "p"), 3.8414709848078967);
    c.set_number(c.id("a").unwrap(), -3.0).unwrap();
    assert_eq!(sample(&c, "h", 0.5), Some(18.5625));
    assert_eq!(num(&c, "w"), 117.0);
}

#[test]
fn a_function_is_undefined_when_an_input_is() {
    let mut c = load(
        "a = 1\nc = a/a\nd = 1/a\nf(x) = x + c\ng(x) = f(x) + 1\nh(x) = x + d\n\
         k(x) = atan(d) + x\nv = f(2)\n",
    )
    .unwrap();
    let a = c.id("a").unwrap();
    c.set_number(a, 0.0).unwrap();
    for label in ["f", "g", "v"] {
        assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined, "{label}");
    }
    // an infinite number is a defined input
    assert!(sample(&c, "h", 1.0).is_some_and(f64::is_infinite));
    assert_eq!(sample(&c, "k", 0.0), Some(std::f64::consts::FRAC_PI_2));
    c.set_number(a, 1.0).unwrap();
    assert_eq!(sample(&c, "g", 1.0), Some(3.0));
    assert_eq!(num(&c, "v"), 3.0);
}

#[test]
fn bad_function_lines_are_rejected() {
    let mut c = load("a = 1\nf(x) = x + a\n").unwrap();
    for bad in [
        "b = f + 1",
        "b = f(1, 2)",
        "g(x) = (x, 1)",
        "g(x) = Line(A, B)",
        "1(x) = x",
        "f(x) = x",
        "g(x) = Mod(x, 3)",
        "b = Max(a, f)",
        "b = GCD((1, 2), a)",
    ] {
        let before = c.len();
        assert!(exec(&mut c, bad).is_err(), "{bad}");
        assert_eq!(c.len(), before, "{bad}");
    }
    // each level reads `x` twice: the expansion doubles until it is too large
    exec(&mut c, "q(x) = x + x").unwrap();
    let mut prev = "q".to_string();
    let mut failure = None;
    for level in 0..40 {
        let name = format!("q{level}");
        let before = (c.len(), c.exprs.len());
        match exec(&mut c, &format!("{name}(x) = {prev}({prev}(x))")) {
            Ok(_) => prev = name,
            Err(e) => {
                assert_eq!((c.len(), c.exprs.len()), before);
                failure = Some((level, e));
                break;
            }
        }
    }
    // The cost guard stops it, not the name or the depth: `x` is read
    // 2^(2^(level + 1)) times, past MAX_COST at level 4.
    let (level, e) = failure.expect("the cost guard never fired");
    assert_eq!(
        (level, e.as_str()),
        (4, "the expression is too large to evaluate")
    );
}

#[test]
fn input_bar_spellings_are_accepted() {
    let c = load(
        "a = 2\n\
         b = 2**3 \u{22c5} a \u{f7} a \u{2212} 1\n\
         c = 7 % 3\n\
         d = 2\u{207b}\u{b9}\u{b2}\n\
         f(x) = sin^2 x\n\
         g(x) = sin\u{b2}(x)\n\
         h = sin^-1(0.5)\n\
         k = floor(h)\n\
         m = cot^-1(1)\n\
         n = a(3)/a\n\
         p = 1E3 + .5 + 5.\n\
         q = x + a\n",
    )
    .unwrap();
    assert_eq!(num(&c, "b"), 7.0);
    assert!((num(&c, "c") - 0.21).abs() < 1e-15);
    assert_eq!(num(&c, "d"), 2f64.powi(-12));
    assert_eq!(sample(&c, "f", 1.0), sample(&c, "g", 1.0));
    // `sin^-1` is an angle: `floor` works in whole degrees
    assert!((num(&c, "k") - 30f64.to_radians()).abs() < 1e-12);
    assert!((num(&c, "m") - std::f64::consts::FRAC_PI_4).abs() < 1e-15);
    assert_eq!(num(&c, "n"), 3.0);
    assert_eq!(num(&c, "p"), 1005.5);
    // `x` outside `f(x) = ...` makes a function
    assert_eq!(c.kind(c.id("q").unwrap()), Kind::Function);
    assert_eq!(sample(&c, "q", 1.0), Some(3.0));
}

#[test]
fn literals_names_and_commands_follow_the_input_bar() {
    let mut c = load("a = 1\n").unwrap();
    // `1e3` is `1·e3`, and `e3` does not exist; `1.2.3` is no number
    for bad in [
        "b = 1e3",
        "b = 1.2.3",
        "b = 1..2",
        "x = 5",
        "y = a",
        "pi = 3",
        "sin = 3",
    ] {
        assert!(exec(&mut c, bad).is_err(), "{bad}");
    }
    assert!(exec(&mut c, "sin(x) = x").is_err());
    // the old forms keep working, and `+5` and `inf` go through expressions
    for (line, label, def) in [
        ("r = 3", "r", Some(Def::Number(3.0))),
        ("s = -2.5", "s", Some(Def::Number(-2.5))),
        ("t = +5", "t", Some(Def::Number(5.0))),
        ("u = inf", "u", None),
    ] {
        exec(&mut c, line).unwrap();
        if let Some(def) = def {
            assert_eq!(c.def(c.id(label).unwrap()), def, "{line}");
        }
    }
    assert_eq!(c.value(c.id("u").unwrap()), Value::Undefined);
    // `e` and `Mod` are labels; `e` then wins over Euler's number
    exec(&mut c, "e = 5").unwrap();
    exec(&mut c, "b = 2e").unwrap();
    assert_eq!(num(&c, "b"), 10.0);
    // a command makes a constant expression dependent: it cannot be set
    exec(&mut c, "m = Mod(7, 3)").unwrap();
    exec(&mut c, "n = 2 + 3").unwrap();
    assert!(c.set_number(c.id("m").unwrap(), 2.0).is_err());
    assert!(c.set_number(c.id("n").unwrap(), 2.0).is_ok());
    // digits that evaluate to `NaN` round to an integer, unless an object
    exec(&mut c, "o = 0").unwrap();
    exec(&mut c, "g = round(2.567, o/o)").unwrap();
    exec(&mut c, "h = round(2.567, ∞)").unwrap();
    assert_eq!(num(&c, "g"), 3.0);
    assert_eq!(c.value(c.id("h").unwrap()), Value::Undefined);
}

#[test]
fn lines_the_input_bar_rejects_are_rejected() {
    // GeoGebra's parser gives "Please check your input" for each of these
    // (checked with the harness).
    for line in ["c = a % b", "c = a%", "c = |a| |b|", "c = 2|a|", "c = a|b|"] {
        let mut c = load("a = 3\nb = 2").unwrap();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), 2, "`{line}` left an object behind");
    }
}

#[test]
fn an_expression_is_rebuilt_from_its_tree_and_never_from_its_handle() {
    let src = "a = 2\nA = (1, 2)\nB = (4, 6)\nc = a + Distance(A, B) / 2\n\
               f(x) = x^2 + c\nv = f(a)\nM = (A + B) / 2\nu = (a; 30\u{b0})\nh = Mod(7, 3)";
    let orig = load(src).unwrap();
    let mut fresh = Construction::new();
    for i in 0..orig.len() {
        let id = crate::kernel::Id(i);
        let added = match orig.expression(id) {
            Some(e) => {
                assert_eq!(e.kind(), orig.kind(id));
                fresh.add_expression(orig.label(id), &e)
            }
            None => fresh.add(orig.label(id), orig.def(id)),
        };
        assert_eq!(added, Ok(id));
        assert_eq!(fresh.value(id), orig.value(id), "`{}`", orig.label(id));
    }
    assert_eq!(fresh.set_number(fresh.id("a").unwrap(), 3.0), Ok(4));
    assert!((num(&fresh, "v") - 14.5).abs() < 1e-12);

    // the handle itself names a tree of `orig`: in another construction, and
    // a second time in its own, it is an error and changes nothing
    let c = orig.id("c").unwrap();
    let mut other = load("a = 2\nb = 5").unwrap();
    assert_eq!(
        other.add("w", orig.def(c)),
        Err(crate::Error::UnknownObject)
    );
    assert_eq!(other.len(), 2);
    let mut same = load(src).unwrap();
    let n = same.len();
    let handle = same.def(same.id("c").unwrap());
    assert_eq!(same.add("w", handle), Err(crate::Error::UnknownObject));
    assert_eq!(same.len(), n);

    // a tree is checked against the construction it is added to
    let e = orig.expression(c).unwrap();
    assert_eq!(
        other.add_expression("w", &e),
        Err(crate::Error::UnknownObject)
    );
    let f = orig.expression(orig.id("f").unwrap()).unwrap();
    let mut numbers = load("a = 2\nb = 3\nC = (1, 1)\nD = (2, 2)\nk = 1").unwrap();
    // `c` (id 3) is a point there, and object 4 is not a function
    assert_eq!(
        numbers.add_expression("w", &f),
        Err(crate::Error::UnknownObject)
    );
    assert_eq!(numbers.len(), 5);
    // a free number keeps the decimal of its literal until it is changed
    let a = orig.id("a").unwrap();
    assert_eq!(orig.expression(a).map(|e| e.kind()), Some(Kind::Number));
    assert_eq!(
        fresh.expression(fresh.id("a").unwrap()).map(|e| e.kind()),
        None
    );
    let added = Construction::new().add_expression("q", &orig.expression(a).unwrap());
    assert_eq!(added, Ok(crate::kernel::Id(0)));
    assert_eq!(
        orig.expression(orig.id("A").unwrap()).map(|e| e.kind()),
        None
    );
}

#[test]
fn decimal_literals_are_exact_until_changed() {
    let mut c = load(
        "a = 0.1
b = a + 0.2 - 0.3
d = 0.1 + 0.2 - 0.3
r = (0.1 + 0.2) 1E20",
    )
    .unwrap();
    assert_eq!(num(&c, "b"), 0.0);
    assert_eq!(num(&c, "d"), 0.0);
    assert_eq!(num(&c, "r"), 3e19);
    // `setValue` drops the decimal even when the value stays: `b` changes
    let a = c.id("a").unwrap();
    assert_eq!(c.set_number(a, 0.1), Ok(1));
    assert_eq!(num(&c, "b"), 0.1 + 0.2 - 0.3);
    assert_eq!(c.set_number(a, 0.1), Ok(0));
    // a literal made through the API has no decimal
    let mut api = Construction::new();
    let x = api.add("a", Def::Number(0.1)).unwrap();
    assert!(api.expression(x).is_none());
    // a hostile exponent falls back to doubles instead of growing
    let h = load(
        "t = 1E-999999999 + 1\nu = 0.999^999999 + 0.5\np = (1E-999999999)^3 + 1\n\
         q = 1E-3000000000 + 1\nr = 1E-999999999 1E-999999999 + 1",
    )
    .unwrap();
    assert_eq!(num(&h, "t"), 1.0);
    // GeoGebra rejects these three lines (exponent outside its `int` scale,
    // out of memory); the kernel computes them with doubles
    for name in ["p", "q", "r"] {
        assert_eq!(num(&h, name), 1.0, "`{name}`");
    }
    assert_eq!(num(&h, "u"), 0.5);
}

#[test]
fn unsupported_syntax_is_an_error_and_leaves_nothing_behind() {
    let base = "a = 2\nA = (1, 2)";
    for line in [
        "b = 30\u{b0}15'",
        "b = 2 rad",
        "b = sin[a]",
        "b = xcoord(A)",
        "B = (1 | 2)",
        "b = A < 3",
        "b = \u{ac}a",
        "b = a && true",
        "b = 1..2",
        "b = random()",
        "b = \u{663}",
        "b = 2\u{966}",
    ] {
        let mut c = load(base).unwrap();
        assert!(exec(&mut c, line).is_err(), "`{line}` was accepted");
        assert_eq!(c.len(), 2, "`{line}` left an object behind");
    }
    // the constants' names are labels
    let c = load("inf = 3\nNaN = 4\nb = inf + NaN").unwrap();
    assert!(matches!(c.value(c.id("b").unwrap()), Value::Undefined));
    assert_eq!(num(&c, "inf"), 3.0);
}

#[test]
fn inverse_or_power_follows_the_brackets_and_constant_exponents() {
    // values from GeoGebra: `(sin²(a))³` and `sin⁻¹(a)` in both cases
    let c = load("a = 0.5\nb = sin^2(a)^3\nd = sin^(1-2)(a)\nk = sin^2((a)^3)").unwrap();
    assert!((num(&c, "b") - 0.01214302779048423).abs() < 1e-15);
    assert!((num(&c, "d") - std::f64::consts::FRAC_PI_6).abs() < 1e-15);
    assert!((num(&c, "k") - 0.125f64.sin().powi(2)).abs() < 1e-15);
}

#[test]
fn an_intersection_hint_is_decided_again_where_the_tree_is_added() {
    // GeoGebra accepts `Intersect(k, k, 1)` inside an expression; it is the
    // point on `k` by construction, here `S`
    let src = "O = (0, 0)\nr = 1\nk = Circle(O, r)\nQ = (1, 1)\nl = Line(O, Q)\n\
               S = Intersect(l, k, 1)\nM = Intersect(k, k, 1) * 1\nm = x(Intersect(k, k, 1))";
    let orig = load(src).unwrap();
    let s = orig.value(orig.id("S").unwrap());
    assert_eq!(orig.value(orig.id("M").unwrap()), s);
    let e = orig.expression(orig.id("M").unwrap()).unwrap();
    // only `O, r, k`: the hint names `S`, which is not there
    let mut small = load("O = (0, 0)\nr = 1\nk = Circle(O, r)").unwrap();
    let m = small.add_expression("M", &e).unwrap();
    exec(&mut small, "N = Intersect(k, k, 1)").unwrap();
    assert_eq!(small.value(m), small.value(small.id("N").unwrap()));
}

#[test]
fn an_applied_function_keeps_the_decimals_of_its_argument() {
    // values from GeoGebra: inside a function `f(1.005)` is `round(1.005 * 100)`
    // on the decimal, elsewhere `f` is sampled at the double 1.005
    let c = load(
        "f(x) = round(x 100)\ng(x) = f(1.005) + x\nv = f(1.005)\nb = 1.005\n\
         k(x) = f(b) + x\nw = f(b)\nq(x) = f(1.005 + 0) + x\nm(x) = x + f(1.005)",
    )
    .unwrap();
    for (name, at0) in [("g", 101.0), ("k", 101.0), ("q", 101.0), ("m", 101.0)] {
        let id = c.id(name).unwrap();
        assert_eq!(c.function_value(id, 0.0), Some(at0), "`{name}`");
        assert_eq!(c.function_value(id, 2.0), Some(at0 + 2.0), "`{name}`");
    }
    assert_eq!(num(&c, "v"), 100.0);
    assert_eq!(num(&c, "w"), 100.0);
}

/// Decimal operations done on this thread while `f` runs.
fn decimal_ops(f: impl FnOnce()) -> u64 {
    let before = super::decimal::OPS.with(std::cell::Cell::get);
    f();
    super::decimal::OPS.with(std::cell::Cell::get) - before
}

#[test]
fn inputs_without_decimals_cost_no_decimal_arithmetic() {
    let mut c = load(
        "A = (1, 2)\nB = (3, 4)\nM = (A + B) / 2\na = 0.3\nc = 0.5 a + x(A)\n\
         f(x) = x^2 + a\nv = f(x(A))\nd = round(x(A) 0.1, 2) + floor(y(A))",
    )
    .unwrap();
    let (a, p) = (c.id("a").unwrap(), c.id("A").unwrap());
    // setting `a` drops its decimal
    c.set_number(a, 0.7).unwrap();
    let f = c.id("f").unwrap();
    let ops = decimal_ops(|| {
        for i in 1..=100 {
            let x = f64::from(i) * 0.01;
            assert!(c.move_point(p, 1.0 + x, 2.0 - x).unwrap() > 0);
            assert!(c.function_value(f, x).is_some());
        }
    });
    assert_eq!(ops, 0);
    // the counter is live: a literal times a literal number is exact
    let mut exact = load("a = 0.1\nA = (1, 2)\nc = 0.5 a + x(A)").unwrap();
    let p = exact.id("A").unwrap();
    assert!(decimal_ops(|| assert!(exact.move_point(p, 3.0, 4.0).unwrap() > 0)) > 0);
}

#[test]
fn an_exponent_with_a_unicode_minus_is_an_error_in_every_line() {
    // GeoGebra's `StringUtil.parseDouble` stops at the dash: "Not a digit"
    let mut c = load("a = 1").unwrap();
    for line in [
        "b = 1.5E\u{2212}3",
        "b = 1E\u{2013}3",
        "P = (1E\u{2212}3, 1)",
        "c = 1.5E\u{2212}3 + 0",
    ] {
        assert!(exec(&mut c, line).is_err(), "`{line}`");
    }
    assert_eq!(c.len(), 1);
}

#[test]
fn setting_a_decimal_number_without_dependents_recomputes_nothing() {
    // The count alone cannot tell that the decimal was dropped: an app that
    // logs sets must also log a set of a number that still had a decimal.
    let mut c = load("a = 0.1").unwrap();
    let a = c.id("a").unwrap();
    assert!(c.expression(a).is_some());
    assert_eq!(c.set_number(a, 0.1), Ok(0));
    assert!(c.expression(a).is_none());
}

fn truth(c: &Construction, label: &str) -> bool {
    match c.value(c.id(label).unwrap()) {
        Value::Bool(b) => b,
        other => panic!("`{label}` is {other:?}"),
    }
}

/// Rebuilds every object of `orig` from its tree or definition and checks
/// that the values agree.
fn rebuilt(orig: &Construction) -> Construction {
    let mut fresh = Construction::new();
    for i in 0..orig.len() {
        let id = crate::kernel::Id(i);
        let added = match orig.expression(id) {
            Some(e) => fresh.add_expression(orig.label(id), &e),
            None => fresh.add(orig.label(id), orig.def(id)),
        };
        assert_eq!(added, Ok(id), "`{}`", orig.label(id));
        assert_eq!(fresh.value(id), orig.value(id), "`{}`", orig.label(id));
        assert_eq!(flat(&fresh, id), flat(orig, id), "`{}`", orig.label(id));
    }
    fresh
}

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
        "f(x) = x < 3",
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
        "b = If(a < 3, t, 1)",
        "b = If(a < 3, 1, a < 4)",
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

/// The elements of list `id` written out, nested lists in braces; `None`
/// when `id` is not a defined list.
fn flat(c: &Construction, id: crate::kernel::Id) -> Option<String> {
    fn write(list: super::List<'_>, out: &mut String) {
        out.push('{');
        for (k, e) in list.iter().enumerate() {
            if k > 0 {
                out.push_str(", ");
            }
            match e {
                super::Element::List(inner) => write(inner, out),
                super::Element::Value(_, Value::Number(v)) => out.push_str(&format!("{v}")),
                super::Element::Value(_, Value::Point { x, y }) => {
                    out.push_str(&format!("({x}, {y})"));
                }
                super::Element::Value(_, Value::Vector { x, y }) => {
                    out.push_str(&format!("v({x}, {y})"));
                }
                super::Element::Value(_, Value::Bool(b)) => out.push_str(&format!("{b}")),
                super::Element::Value(kind, _) => out.push_str(&format!("?{kind}")),
            }
        }
        out.push('}');
    }
    let list = c.elements(id)?;
    let mut out = String::new();
    write(list, &mut out);
    Some(out)
}

fn elems(c: &Construction, label: &str) -> String {
    flat(c, c.id(label).unwrap()).unwrap_or_else(|| "undefined".into())
}

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
    let before = super::work_done();
    let r = f();
    (r, super::work_done() - before)
}

/// The most work one evaluation past the element bound does here: at most
/// [`super::MAX_ELEMENTS`] elements built, each with a step of counting,
/// one of filling and a small body (up to 3 nodes visited, 3 units each),
/// and a range counted within what is left.
const WORK_BOUND: u64 = 12 * super::MAX_ELEMENTS;

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
const PAST_WORK_BOUND: u64 = super::MAX_EVALUATION_WORK + super::MAX_OVERRUN;

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
            super::MAX_EVALUATION_WORK,
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
        let free = super::eval::unbounded(|| load(&script).unwrap());
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
        let free = super::eval::unbounded(|| load(&script).unwrap());
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
