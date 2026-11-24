use crate::kernel::{Construction, Def, Kind, Value};
use crate::script::{exec, load};

pub(super) fn num(c: &Construction, label: &str) -> f64 {
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
fn an_infinite_number_is_a_defined_value() {
    // `GeoNumeric.isDefined` only refuses NaN (golden group `infinity`)
    let mut c = load("a = 2\nc = 1/a\nd = 1/c\nf = atan(c)\ng = c - c\n").unwrap();
    let a = c.id("a").unwrap();
    c.set_number(a, 0.0).unwrap();
    assert_eq!(num(&c, "c"), f64::INFINITY);
    assert_eq!(c.infinite_number(c.id("c").unwrap()), Some(f64::INFINITY));
    assert_eq!(c.value(c.id("g").unwrap()), Value::Undefined);
    assert_eq!(num(&c, "d"), 0.0);
    assert_eq!(num(&c, "f"), std::f64::consts::FRAC_PI_2);
    c.set_number(a, -0.0).unwrap();
    assert_eq!(num(&c, "c"), f64::NEG_INFINITY);
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
        "a = a + 2",
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

pub(super) fn sample(c: &Construction, label: &str, x: f64) -> Option<f64> {
    c.function_value(c.id(label).unwrap(), x)
}

#[test]
fn inverse_chains_are_bounded_at_creation_update_and_sampling() {
    let mut c = load("n = 2\ng0(x) = x + n\ng1 = Invert(g0)\ng2 = Invert(g1)").unwrap();
    let before = super::work_done();
    exec(&mut c, "g3 = Invert(g2)").unwrap();
    assert!(super::work_done() > before, "inversion was not metered");

    let mut last = 3;
    for index in 4..=50 {
        let before = super::work_done();
        let len = c.len();
        match exec(&mut c, &format!("g{index} = Invert(g{last})")) {
            Ok(_) => last = index,
            Err(error) => {
                assert!(error.contains("the expression is too large to evaluate"));
                assert_eq!(c.len(), len);
            }
        }
        assert!(super::work_done() - before <= super::MAX_EVALUATION_WORK + super::MAX_OVERRUN);
    }
    assert!(last < 50, "the static cost did not stop the chain");
    let inline = format!("g = {}x{}", "Invert(".repeat(50), ")".repeat(50));
    assert!(exec(&mut c, &inline)
        .unwrap_err()
        .contains("the expression is too large to evaluate"));

    let last_label = format!("g{last}");
    let n = c.id("n").unwrap();
    let before = super::work_done();
    super::eval::with_work_limit(10, || c.set_number(n, 4.0)).unwrap();
    assert!(!c.value(c.id(&last_label).unwrap()).is_defined());
    assert!(super::work_done() > before);

    c.set_number(n, 5.0).unwrap();
    assert_eq!(sample(&c, "g2", 7.0), Some(12.0));
    assert_eq!(sample(&c, "g3", 7.0), Some(2.0));
    let before = super::work_done();
    assert!(sample(&c, &last_label, 7.0).is_some_and(f64::is_finite));
    assert!((1..=super::MAX_EVALUATION_WORK + super::MAX_OVERRUN)
        .contains(&(super::work_done() - before)));
    assert!(
        super::eval::with_work_limit(10, || sample(&c, &last_label, 7.0)).is_some_and(f64::is_nan)
    );
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
        "f(x) = f(x)",
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
        "b = 1..",
        "x = 5",
        "y = a",
        "pi = 3",
        "sin = 3",
    ] {
        assert!(exec(&mut c, bad).is_err(), "{bad}");
    }
    assert!(exec(&mut c, "sin(x) = x").is_err());
    exec(&mut c, "r_{range} = 1..2").unwrap();
    assert_eq!(c.value(c.id("r_{range}").unwrap()), Value::List { len: 2 });
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
    assert_eq!(c.value(c.id("u").unwrap()), Value::Number(f64::INFINITY));
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
        "b = 2 ** ..",
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
pub(super) fn decimal_ops(f: impl FnOnce()) -> u64 {
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

pub(super) fn truth(c: &Construction, label: &str) -> bool {
    match c.value(c.id(label).unwrap()) {
        Value::Bool(b) => b,
        other => panic!("`{label}` is {other:?}"),
    }
}

/// Rebuilds every object of `orig` from its tree or definition and checks
/// that the values agree.
pub(super) fn rebuilt(orig: &Construction) -> Construction {
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

/// The elements of list `id` written out, nested lists in braces; `None`
/// when `id` is not a defined list.
pub(super) fn flat(c: &Construction, id: crate::kernel::Id) -> Option<String> {
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
                super::Element::Text(s) => out.push_str(s),
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

pub(super) fn elems(c: &Construction, label: &str) -> String {
    flat(c, c.id(label).unwrap()).unwrap_or_else(|| "undefined".into())
}

#[test]
fn histogram_respects_the_work_allowance_for_named_lists() {
    let mut c =
        crate::script::load("borders = Sequence(0, 100)\ndata = Sequence(1, 100)\ndensity = true")
            .unwrap();
    let before = super::work_done();
    let id = super::eval::with_work_limit(600, || {
        crate::script::exec(&mut c, "h = Histogram(borders, data, density)")
            .unwrap()
            .unwrap()
    });
    assert_eq!(c.value(id), crate::Value::Undefined);
    assert!(super::work_done() - before <= 600);
}

#[test]
fn dilate_bounds_the_search_for_the_first_leaf() {
    let mut c = load("A = (1, 2)\nl = {Sequence({}, i, 1, 200), {1, A}}").unwrap();
    let before = super::work_done();
    let id = super::eval::with_work_limit(20, || {
        crate::script::exec(&mut c, "p = Dilate(l, 2)")
            .unwrap()
            .unwrap()
    });
    assert_eq!(c.value(id), Value::Undefined);
    assert!(super::work_done() - before <= 40);
    assert!(crate::script::exec(&mut c, "q = Dilate(l, 2)").is_err());
    assert!(c.id("q").is_none());
}

#[test]
fn the_undefined_constant_alone_is_an_undefined_function() {
    // golden topic `tpairfixes-fnundef`
    let c = load(
        "f(x) = ?\ng(x) = f(x) + 1\nh(x) = 0 / 0\nk(x) = ? + x\nm(x) = (?)\nr = f(1)\n\
         b = IsDefined(f)\nd = IsDefined(h)\n",
    )
    .unwrap();
    let value = |label: &str| c.value(c.id(label).unwrap());
    assert_eq!(value("f"), Value::Undefined);
    assert_eq!(value("g"), Value::Undefined);
    assert_eq!(value("m"), Value::Undefined);
    // every other function without a value is defined
    assert_eq!(value("h"), Value::Function);
    assert_eq!(value("k"), Value::Function);
    assert_eq!(value("r"), Value::Undefined);
    assert_eq!(value("b"), Value::Bool(false));
    assert_eq!(value("d"), Value::Bool(true));
}

#[test]
fn arithmetic_is_told_from_trees_the_evaluation_does_not_run() {
    let c = load("a = -1\nA = (2, 3)\nQ = (sqrt(a), 0)\nl = {1, 2}").unwrap();
    // Arithmetic over the construction, with or without a value right now.
    assert_eq!(c.arithmetic_value("x(A) / 2 + 1"), Some(2.0));
    assert!(c.arithmetic_value("sqrt(a)").unwrap().is_nan());
    assert!(c.arithmetic_value("x(Q) / 10").unwrap().is_nan());
    assert!(c.arithmetic_value("If(a > 0, 1, ?)").unwrap().is_nan());
    // Trees the evaluation of a single number leaves undefined whatever the
    // inputs, and what does not parse as a number at all.
    for source in [
        "Sum(Sequence(k, k, 1, 10))",
        "Length(l)",
        "Sum(l)",
        "x",
        "A",
        "SetValue(a, 3)",
        "a := 3",
        "missing + 1",
    ] {
        assert_eq!(c.arithmetic_value(source), None, "{source}");
    }
    // The value is the one `numeric_value` gives.
    assert_eq!(c.numeric_value("x(A) / 2 + 1"), Some(2.0));
}

#[test]
fn shared_deadline_counts_plain_scalar_expression_visits() {
    let c = crate::script::load("a = 2").unwrap();
    let before = super::work_done();
    super::with_work_deadline(before, || {
        assert!(c.arithmetic_value("sqrt(a) + sqrt(a)").unwrap().is_nan());
    });
    assert!(super::work_done() > before);
    assert_eq!(
        c.arithmetic_value("sqrt(a) + sqrt(a)"),
        Some(2.0_f64.sqrt() * 2.0)
    );
}

#[test]
fn defined_function_with_small_work_limit() {
    let mut c = crate::script::load("f(x) = x + x").unwrap();
    let result = super::eval::with_work_limit(1, || crate::script::exec(&mut c, "b = Defined(f)"))
        .unwrap()
        .unwrap();
    assert_eq!(c.value(result), crate::Value::Bool(true));
}

#[test]
fn a_zero_degree_term_keeps_a_function_defined() {
    // GeoGebra prints `? + 0°` for this function, which is not `?`.
    let mut c = crate::script::load("f(x) = ? + 0° + 0x").unwrap();
    let result = crate::script::exec(&mut c, "b = Defined(f)")
        .unwrap()
        .unwrap();
    assert_eq!(c.value(result), crate::Value::Bool(true));
}

#[test]
fn conditional_arc_carrier_respects_the_work_limit() {
    let mut c =
        crate::script::load("O = (1, 2)\nA = (2, 2)\nB = (1, 3)\nr0 = CircleArc(O, A, B)").unwrap();
    for i in 1..=64 {
        let previous = i - 1;
        crate::script::exec(
            &mut c,
            &format!("r{i} = If(true, r{previous}, r{previous})"),
        )
        .unwrap();
    }
    let before = super::work_done();
    let r = c.id("r64").unwrap();
    assert!(super::eval::with_work_limit(4, || c.arc(r)).is_none());
    assert!(super::work_done() - before <= 4);
    assert!(c.arc(r).is_some());
}

#[test]
fn defined_function_reuses_the_callers_work_budget() {
    let source = format!("f(x) = x{}", " + x".repeat(24));
    let c = crate::script::load(&source).unwrap();
    let id = c.id("f").unwrap();
    let crate::Def::Expr(def) = c.def(id) else {
        panic!("expected a function expression");
    };
    let entry = super::entry(&c, def);
    super::eval::with_work_limit(16, || {
        let meter = super::eval::Lists::new(Vec::new());
        assert!(meter.charge(14));
        let ctx = super::eval::Ctx::<true> {
            meter: Some(&meter),
            ..super::eval::Ctx::new(&c, entry, &super::eval::Env::None)
        };
        let before = super::work_done();
        assert_eq!(super::commands::defined_object(&ctx, id), None);
        assert!(super::work_done() - before <= 3);
    });
}

#[test]
fn exhausted_dilation_validation_refuses_the_command() {
    let mut c = load("A = (1, 1)").unwrap();
    let zeros = vec!["0"; 64].join(", ");
    let source = format!("q = If(false, Length({{{zeros}}}) + Length(Dilate({{1, A}}, 2)), 7)");
    let before = super::work_done();
    let result = super::eval::with_work_limit(32, || crate::script::exec(&mut c, &source));
    assert!(result.is_err());
    assert!(c.id("q").is_none());
    assert!(super::work_done() - before <= 32);
}

#[test]
fn list_dilation_respects_the_remaining_carrier_budget() {
    let mut c = load("r0 = Circle((1, 2), 2)").unwrap();
    for i in 1..=64 {
        let previous = i - 1;
        crate::script::exec(
            &mut c,
            &format!("r{i} = If(true, r{previous}, r{previous})"),
        )
        .unwrap();
    }
    let source = "n = Length(Dilate({r64}, 2))";
    let before = super::work_done();
    let n = super::eval::with_work_limit(32, || crate::script::exec(&mut c, source))
        .unwrap()
        .unwrap();
    assert_eq!(c.value(n), Value::Undefined);
    assert!(super::work_done() - before <= 33);
}

#[test]
fn a_scalar_replacing_a_nested_list_leaves_its_points_at_the_previous_scale() {
    use super::Element;

    for command in ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"] {
        let mut c = crate::script::load(&format!(
            "t = 1\nA = (1, 2)\nB = (3, 5)\nL = If(t > 0, {{{{A}}, B}}, {{3, B}})\nM = {command}"
        ))
        .unwrap();
        c.set_number(c.id("t").unwrap(), 0.0).unwrap();
        let result = c.elements(c.id("M").unwrap()).unwrap();
        let Some(Element::List(nested)) = result.get(0) else {
            panic!("expected a retained nested list");
        };
        assert!(matches!(
            nested.get(0),
            Some(Element::Value(Kind::Point, Value::Point { x: 2.0, y: 4.0 }))
        ));
        assert!(matches!(
            result.get(1),
            Some(Element::Value(
                Kind::Point,
                Value::Point { x: 6.0, y: 10.0 }
            ))
        ));
    }
}

#[test]
fn followup_nested_transform_keeps_the_active_point_across_two_scalar_replacements() {
    use super::Element;

    for command in ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"] {
        let mut c = load(&format!(
            "t = 2\nA = (1, 2)\nB = (3, 5)\n\
             L = If(t > 1, {{{{A}}, {{B}}}}, If(t > 0, {{{{A}}, 4}}, {{3, 4}}))\nM = {command}"
        ))
        .unwrap();
        let t = c.id("t").unwrap();
        c.set_number(t, 1.0).unwrap();
        c.set_number(t, 0.0).unwrap();
        let result = c.elements(c.id("M").unwrap()).unwrap();
        for (index, expected) in [
            (0, Value::Point { x: 16.0, y: 32.0 }),
            (1, Value::Point { x: 6.0, y: 10.0 }),
        ] {
            let Some(Element::List(nested)) = result.get(index) else {
                panic!("expected a retained nested list at {index}");
            };
            assert!(
                matches!(
                    nested.get(0),
                    Some(Element::Value(Kind::Point, point)) if point == expected
                ),
                "{command}, output {index}: {:?}",
                nested.get(0)
            );
        }
    }
}

#[test]
fn followup_a_redefined_direct_endpoint_notifies_before_the_other_bound_updates() {
    let mut c = load("u = -4\nhi = u + 3\na = Slider(u, hi)").unwrap();
    crate::script::exec(&mut c, "u = -0.5").unwrap();
    assert_eq!(c.value(c.id("a").unwrap()), Value::Number(1.0));
}

#[test]
fn followup_an_omitted_infinite_step_stays_frozen_after_widening() {
    let mut c = load("h = 0.2\na = Slider(0, 6, h, 2)").unwrap();
    let a = c.id("a").unwrap();
    c.set_number(a, 5.0).unwrap();
    crate::script::exec(&mut c, "h = 1/(a - 2)").unwrap();
    crate::script::exec(&mut c, "a = Slider(0, 2)").unwrap();
    crate::script::exec(&mut c, "a = Slider(0, 6)").unwrap();
    c.set_number(a, 5.0).unwrap();
    let step = c.slider_animation_options(a)[0].unwrap();
    assert_eq!(c.numeric_value(step), Some(f64::INFINITY));
}

const ROUND2_TRANSFORMS: [&str; 2] = ["Dilate(L, 2)", "ApplyMatrix({{2, 0}, {0, 2}}, L)"];
const ROUND2_NESTED: &str =
    "t = 2\nA = (1, 2)\nB = (3, 5)\nL = If(t > 1, {{A}, {B}}, If(t > 0, {{A}, 4}, {3, 4}))";

fn first_nested_x(c: &Construction) -> f64 {
    let result = c.elements(c.id("M").unwrap()).unwrap();
    let Some(super::Element::List(nested)) = result.get(0) else {
        panic!("expected nested list");
    };
    let Some(super::Element::Value(Kind::Point, Value::Point { x, .. })) = nested.get(0) else {
        panic!("expected point");
    };
    x
}

#[test]
fn round2_value_stable_selector_updates_the_active_transform() {
    for command in ROUND2_TRANSFORMS {
        for source in [
            ROUND2_NESTED.to_owned(),
            ROUND2_NESTED.replace("{3, 4}))", "If(t == 0, {3, 4}, {5, 6})))"),
        ] {
            let mut c = load(&format!("{source}\nM = {command}")).unwrap();
            let t = c.id("t").unwrap();
            c.set_number(t, 1.0).unwrap();
            assert_eq!(first_nested_x(&c), 4.0);
            c.set_number(t, 0.0).unwrap();
            assert_eq!(first_nested_x(&c), 16.0);
            c.set_number(t, -1.0).unwrap();
            assert_eq!(first_nested_x(&c), 64.0);
        }
    }
}

#[test]
fn round2_structural_rebuild_restarts_nested_transform_slots() {
    for command in ROUND2_TRANSFORMS {
        let mut c = load(&format!(
            "{ROUND2_NESTED}\nM = {command}\nq = 10\nw = q + 1"
        ))
        .unwrap();
        c.set_number(c.id("t").unwrap(), 1.0).unwrap();
        crate::script::exec(&mut c, "q = x(A)").unwrap();
        assert_eq!(first_nested_x(&c), 4.0);
        assert!(matches!(
            c.elements(c.id("M").unwrap()).unwrap().get(1),
            Some(super::Element::Value(Kind::Number, Value::Number(4.0)))
        ));
    }
}

#[test]
fn round2_empty_list_keeps_the_active_transform() {
    for command in ROUND2_TRANSFORMS {
        let mut c = load(&format!(
            "t = 2\nA = (1, 2)\nL = If(t > 1, {{{{A}}}}, If(t > 0, {{}}, {{3}}))\nM = {command}"
        ))
        .unwrap();
        let (t, m) = (c.id("t").unwrap(), c.id("M").unwrap());
        c.set_number(t, 1.0).unwrap();
        assert_eq!(c.elements(m).unwrap().len(), 0);
        c.set_number(t, 0.0).unwrap();
        let list = c.elements(m).unwrap();
        assert_eq!(list.len(), 1);
        assert!(matches!(
            list.get(0),
            Some(super::Element::Value(Kind::Number, Value::Number(3.0)))
        ));
    }
}

#[test]
fn round2_slider_dependents_observe_transient_undefined() {
    let mut c = load(
        "u = -4\nhi = u + 3\na = Slider(u, hi)\nb = IsDefined(a)\nP = (1, 2)\nQ = (3, 5)\n\
         L = If(b, If(a < 0, {{P}, {Q}}, {3, 4}), {{P}, 4})\nM = Dilate(L, 2)",
    )
    .unwrap();
    crate::script::exec(&mut c, "u = -0.5").unwrap();
    assert_eq!(first_nested_x(&c), 16.0);
}

#[test]
fn round2_omitted_nan_step_stays_frozen() {
    let mut c = load("h = 0.2\na = Slider(0, 6, h, 2)").unwrap();
    let a = c.id("a").unwrap();
    c.set_number(a, 5.0).unwrap();
    crate::script::exec(&mut c, "h = sqrt(a - 3)").unwrap();
    crate::script::exec(&mut c, "a = Slider(0, 2)").unwrap();
    crate::script::exec(&mut c, "a = Slider(0, 6)").unwrap();
    c.set_number(a, 5.0).unwrap();
    assert_eq!(c.slider_animation_options(a)[0], Some("NaN"));
}
