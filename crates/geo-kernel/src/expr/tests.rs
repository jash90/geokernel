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
    assert_eq!(
        orig.expression(orig.id("a").unwrap()).map(|e| e.kind()),
        None
    );
}
