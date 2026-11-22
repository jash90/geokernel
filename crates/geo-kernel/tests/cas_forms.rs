//! The equation and polynomial commands on lines, conics, curves and texts,
//! for what the golden cases cannot record: results a list cannot hold,
//! display strings, refused inputs and recovery after an undefined parent.

use geo_kernel::expr::Element;
use geo_kernel::{script, Construction, Id, Kind, Value};

fn build(lines: &[&str]) -> Construction {
    let mut c = Construction::new();
    for line in lines {
        script::exec(&mut c, line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
    c
}

fn id(c: &Construction, label: &str) -> Id {
    c.id(label).unwrap_or_else(|| panic!("no object `{label}`"))
}

/// The numbers of a list of numbers, `None` for an undefined list.
fn numbers(c: &Construction, label: &str) -> Option<Vec<f64>> {
    let list = c.elements(id(c, label))?;
    Some(
        list.iter()
            .map(|e| match e {
                Element::Value(Kind::Number, Value::Number(v)) => v,
                other => panic!("{label} holds {other:?}"),
            })
            .collect(),
    )
}

fn samples(c: &Construction, label: &str, k: usize, xs: &[f64]) -> Vec<f64> {
    let list = c.elements(id(c, label)).expect("a defined list");
    xs.iter()
        .map(|&x| list.function_sample_at(c, k, x).expect("a function"))
        .collect()
}

fn rejected(c: &mut Construction, line: &str) -> String {
    let before = c.len();
    let error = script::exec(c, line).expect_err(line);
    assert_eq!(c.len(), before, "`{line}` left objects behind");
    error
}

#[test]
fn a_proper_conic_solved_for_x_is_undefined() {
    // The reference answers implicit curves `x = -sqrt(4 - y^2)`, which a
    // list cannot hold.
    let c = build(&["k: x^2+y^2=4", "r = Solve(k)", "s = Solve(k, x)"]);
    assert!(c.elements(id(&c, "r")).is_none());
    assert!(c.elements(id(&c, "s")).is_none());
    // A parabola solved for its linear variable is the conic itself.
    let c = build(&["k: y=x^2+x", "r = Solve(k, y)"]);
    assert!(c.elements(id(&c, "r")).is_none());
    // Two branches come in an order the reference's CAS chooses: undefined.
    let c = build(&["k: x^2+y^2=4", "r = Solve(k, y)", "s = Solutions(k, y)"]);
    assert!(c.elements(id(&c, "r")).is_none());
    assert!(c.elements(id(&c, "s")).is_none());
    // One branch is answered.
    let c = build(&["k: x*y=1", "r = Solve(k, y)", "s = Solutions(k, y)"]);
    assert_eq!(samples(&c, "r", 0, &[4.0]), [0.25]);
    assert_eq!(samples(&c, "s", 0, &[4.0]), [0.25]);
}

#[test]
fn the_branch_of_a_conic_shows_its_closed_form() {
    let c = build(&["k: x*y=1", "r = Solutions(k, y)"]);
    let list = c.elements(id(&c, "r")).unwrap();
    assert_eq!(list.symbolic_at(0).as_deref(), Some("(1)/(x)"));
}

#[test]
fn a_product_of_lines_is_answered_only_with_one_branch() {
    let mut c = build(&[
        "a = 3",
        "k: (13*x-10*y-10)*(a*x-5*y+2)=0",
        "r = Solutions(k, x)",
    ]);
    assert!(c.elements(id(&c, "r")).is_none());
    // With a = 0 the second factor holds no x: x = (10 y + 10) / 13.
    c.set_number(id(&c, "a"), 0.0).unwrap();
    assert_eq!(c.elements(id(&c, "r")).unwrap().len(), 1);
    assert!((samples(&c, "r", 0, &[0.0])[0] - 10.0 / 13.0).abs() < 1e-15);
    c.set_number(id(&c, "a"), 6.5).unwrap();
    assert!(c.elements(id(&c, "r")).is_none());
}

#[test]
fn a_conic_follows_its_parents_through_every_type() {
    let mut c = build(&[
        "a = 4",
        "k: x^2+y^2=a",
        "r = Solutions(k)",
        "n = NSolutions(k)",
    ]);
    // Two branches: undefined.
    assert!(c.elements(id(&c, "r")).is_none());
    // An empty conic and a single point have no solution, then it is
    // undefined again.
    for value in [-1.0, 0.0] {
        c.set_number(id(&c, "a"), value).unwrap();
        assert_eq!(c.elements(id(&c, "r")).unwrap().len(), 0);
    }
    c.set_number(id(&c, "a"), 9.0).unwrap();
    assert!(c.elements(id(&c, "r")).is_none());
}

#[test]
fn an_initial_value_selects_one_root() {
    let mut c = build(&[
        "b = 2",
        "f(x) = x^3+x-b",
        "g(x) = x^2-3",
        "r = NSolutions(f, x=b)",
        "s = Solve(f, x=b)",
        "u = NSolutions(g, x=b)",
    ]);
    let single = |c: &Construction| {
        let [value] = numbers(c, "r").expect("a defined list")[..] else {
            panic!("expected one root");
        };
        value
    };
    // The only real root of x^3 + x - 2 is 1, from any start.
    assert!((single(&c) - 1.0).abs() < 1e-12);
    assert_eq!(c.elements(id(&c, "s")).unwrap().len(), 1);
    c.set_number(id(&c, "b"), 10.0).unwrap();
    assert!((single(&c) - 2.0).abs() < 1e-12);
    // Two real roots: the root depends on the search, undefined.
    assert!(numbers(&c, "u").is_none());
}

#[test]
fn an_implicit_curve_is_solved_like_its_equation() {
    let c = build(&[
        "k: x^3-x=0",
        "j: x^2*y=1",
        "i: x^3+y^3=1",
        "r1 = Solutions(k)",
        "r2 = NSolutions(k, x=2)",
        "r3 = Solve({k}, x)",
        "r4 = NSolutions({k}, x)",
        "r5 = Solutions(j, y)",
        "r6 = Solutions(i, x)",
        "r7 = PlotSolve(k)",
    ]);
    assert_eq!(c.kind(id(&c, "k")), Kind::ImplicitCurve);
    assert_eq!(numbers(&c, "r1").unwrap(), [-1.0, 0.0, 1.0]);
    // Three real roots: a start value does not decide which one.
    assert!(numbers(&c, "r2").is_none());
    assert_eq!(c.elements(id(&c, "r3")).unwrap().len(), 1);
    assert_eq!(c.elements(id(&c, "r4")).unwrap().len(), 0);
    assert_eq!(samples(&c, "r5", 0, &[2.0]), [0.25]);
    // A cube root is beyond the bounded CAS: undefined, as recorded.
    assert!(c.elements(id(&c, "r6")).is_none());
    assert_eq!(c.elements(id(&c, "r7")).unwrap().len(), 3);
}

#[test]
fn rewriting_an_implicit_curve_is_refused() {
    // The reference answers an implicit curve with an equation text of its
    // CAS; the forms stay open.
    let mut c = build(&["k: x^3+y^3=1"]);
    for command in ["Expand", "Factor", "IFactor", "Simplify"] {
        rejected(&mut c, &format!("r = {command}(k)"));
    }
    for line in ["r = Simplify(x^3+y^3=1)", "r = Simplify(x^3-x*y^2=0)"] {
        rejected(&mut c, line);
    }
}

#[test]
fn a_text_with_an_equals_sign_is_a_text() {
    let c = build(&[
        "r1 = Simplify(\"\u{3b1}=\u{3b2}\")",
        "r2 = Simplify(\"x+-1x=2\")",
        "r3 = Simplify(\"a,b=c\")",
        "r4 = Simplify(\"(a=b\")",
    ]);
    assert_eq!(c.text(id(&c, "r1")), Some("\u{3b1} = \u{3b2}"));
    assert_eq!(c.text(id(&c, "r2")), Some("x \u{2212} x = 2"));
    assert_eq!(c.text(id(&c, "r3")), Some("a,b = c"));
    assert_eq!(c.text(id(&c, "r4")), Some("(a = b"));
}

#[test]
fn a_coordinate_the_kernel_cannot_rewrite_like_the_reference_is_undefined() {
    // The reference's CAS rewrites these coordinates (a cancelled pole past
    // the degree bound, sqrt(t)^2 to t, exp(t) exp(-t) to 1); the kernel
    // rewrites rational coordinates only.
    let c = build(&[
        "k = Curve((t^65-1)/(t-1), t, t, 0, 2)",
        "e = Expand(k)",
        "j = Curve(sqrt(t)^2, t, t, -1, 1)",
        "s = Simplify(j)",
        "i = Curve(exp(t)*exp(-t), t, t, 700, 800)",
        "u = Simplify(i)",
        "h = Curve(sin(t)^2+t, cos(t)*t^2, t, 0, 2)",
        "v = Factor(h)",
    ]);
    for label in ["e", "s", "u", "v"] {
        assert_eq!(c.value(id(&c, label)), Value::Undefined, "{label}");
        assert_eq!(curve_point(&c, label, 0.5), None, "{label}");
    }
    // A polynomial past the bound of the expansion is no exact rewrite
    // either.
    let c = build(&["k = Curve(t^70, t, t, 0, 1)", "e = Expand(k)"]);
    assert_eq!(curve_point(&c, "e", 0.5), None);
}

#[test]
fn simplify_reads_a_typed_line_as_typed() {
    let c = build(&[
        "g: y=x/3",
        "r = Simplify(g)",
        "h: 3y=x+1",
        "s = Simplify(h)",
    ]);
    let r = id(&c, "r");
    assert_eq!(c.function_value(r, 1.0), Some(1.0 / 3.0));
    assert_eq!(c.function_value(id(&c, "s"), 1.0), Some(2.0 / 3.0));
}

#[test]
fn simplify_of_a_line_through_fractions_is_defined() {
    let c = build(&[
        "A = (1/3, 2/7)",
        "B = (5/11, 1/13)",
        "g = Line(A, B)",
        "r = Simplify(g)",
    ]);
    let r = id(&c, "r");
    assert_eq!(c.value(r), Value::Function);
    let (ax, ay, bx, by) = (1.0 / 3.0, 2.0 / 7.0, 5.0 / 11.0, 1.0 / 13.0);
    let slope = (by - ay) / (bx - ax);
    let at = |x: f64| c.function_value(r, x).unwrap();
    assert!((at(0.0) - (ay - slope * ax)).abs() < 1e-12);
    assert!((at(1.0) - (ay + slope * (1.0 - ax))).abs() < 1e-12);
}

#[test]
fn hints_that_are_no_variable_or_value_are_refused() {
    let mut c = build(&["f(x) = x^2-3", "g: x+y=1"]);
    for line in [
        "r = Solve(f, x>0)",
        "r = NSolve(f, {x=1})",
        "r = NSolutions(f, 1)",
        "r = Solve(f, x, y)",
        "r = NSolve(f, x=g)",
        "r = Solutions(g, x=)",
    ] {
        rejected(&mut c, line);
    }
}

#[test]
fn solving_stays_within_its_budget() {
    // A start far away answers the only real root without iterating; a
    // degree past the bound and a third variable are undefined, not slow.
    let c = build(&[
        "f(x) = x^3+1",
        "g(x) = x^3-x",
        "r = NSolutions(f, x=10^300)",
        "s = NSolutions(g, x=1000)",
    ]);
    assert_eq!(numbers(&c, "r").unwrap(), [-1.0]);
    assert!(numbers(&c, "s").is_none());
    let c = build(&["r = Solutions(x^3+y^3=1, x)", "s = Solutions(x+y+z=1, x)"]);
    assert!(c.elements(id(&c, "r")).is_none());
    assert!(c.elements(id(&c, "s")).is_none());
}

fn curve_point(c: &Construction, label: &str, t: f64) -> Option<(f64, f64)> {
    c.curve_point(id(c, label), t)
}

#[test]
fn a_rewritten_curve_keeps_its_points_and_loses_removable_poles() {
    let c = build(&[
        "k = Curve((t^2-1)/(t-1), t^2, t, 0, 2)",
        "e = Expand(k)",
        "f = Factor(k)",
        "i = IFactor(k)",
    ]);
    assert_eq!(curve_point(&c, "k", 1.0), None);
    for label in ["e", "f", "i"] {
        assert_eq!(curve_point(&c, label, 1.0), Some((2.0, 1.0)));
        assert_eq!(curve_point(&c, label, 0.5), Some((1.5, 0.25)));
        assert_eq!(c.value(id(&c, label)), c.value(id(&c, "k")));
    }
}

#[test]
fn a_rewritten_curve_follows_its_source() {
    let mut c = build(&[
        "a = 2",
        "k = Curve(a*t^2, (t+a)^2, t, 0, a)",
        "e = Expand(k)",
    ]);
    assert_eq!(curve_point(&c, "e", 1.0), Some((2.0, 9.0)));
    // An interval that ends before it starts: undefined with its source.
    c.set_number(id(&c, "a"), -1.0).unwrap();
    assert_eq!(c.value(id(&c, "e")), Value::Undefined);
    assert_eq!(curve_point(&c, "e", 0.0), None);
    c.set_number(id(&c, "a"), 3.0).unwrap();
    assert_eq!(curve_point(&c, "e", 1.0), Some((3.0, 16.0)));
    assert_eq!(
        c.value(id(&c, "e")),
        Value::Curve {
            start: 0.0,
            end: 3.0
        }
    );
}

#[test]
fn objects_that_are_no_expression_are_refused() {
    let mut c = build(&[
        "g: x+y=1",
        "k: x^2+y^2=4",
        "l = {1, 2}",
        "A = (0, 0)",
        "B = (1, 2)",
        "C = (3, 1)",
        "s = Spline({A, B, C})",
    ]);
    for command in ["Expand", "Factor", "IFactor"] {
        for argument in ["g", "k", "l", "A", "{1, 2}", "x+y=1"] {
            rejected(&mut c, &format!("r = {command}({argument})"));
        }
        // A spline has no coordinate functions to rewrite.
        let error = rejected(&mut c, &format!("r = {command}(s)"));
        assert!(error.contains("coordinate functions"), "{error}");
        rejected(&mut c, &format!("r = {command}(s, 2)"));
    }
}

fn function_values(c: &Construction, label: &str, xs: &[f64]) -> Vec<Option<f64>> {
    xs.iter()
        .map(|&x| c.function_value(id(c, label), x).filter(|v| v.is_finite()))
        .collect()
}

#[test]
fn simplify_reads_a_line_as_its_function_and_recovers() {
    let mut c = build(&[
        "A = (0, 1)",
        "B = (1, 0)",
        "g = Line(A, B)",
        "r = Ray(A, B)",
        "f = Simplify(g)",
        "h = Simplify(r)",
    ]);
    let b = id(&c, "B");
    for label in ["f", "h"] {
        assert_eq!(
            function_values(&c, label, &[0.0, 3.0]),
            [Some(1.0), Some(-2.0)]
        );
    }
    // Vertical: a function without values, still a defined object.
    c.move_point(b, 0.0, 5.0).unwrap();
    for label in ["f", "h"] {
        assert_eq!(c.value(id(&c, label)), Value::Function);
        assert_eq!(function_values(&c, label, &[0.0, 3.0]), [None, None]);
    }
    c.move_point(b, 2.0, 2.0).unwrap();
    assert_eq!(
        function_values(&c, "f", &[0.0, 2.0]),
        [Some(1.0), Some(2.0)]
    );
}

#[test]
fn simplify_reads_only_a_parabola_in_y_as_a_function() {
    let mut c = build(&["a = 0", "k: x^2+a*y^2+y=1", "f = Simplify(k)"]);
    assert_eq!(
        function_values(&c, "f", &[0.0, 2.0]),
        [Some(1.0), Some(-3.0)]
    );
    c.set_number(id(&c, "a"), 2.0).unwrap();
    assert_eq!(c.value(id(&c, "f")), Value::Undefined);
    c.set_number(id(&c, "a"), 0.0).unwrap();
    assert_eq!(function_values(&c, "f", &[1.0]), [Some(0.0)]);
    // The result is a function like any other.
    let c = build(&[
        "k: y=x^2-1",
        "f = Simplify(k)",
        "g(x) = f(x)+1",
        "r = Root(f)",
    ]);
    assert_eq!(function_values(&c, "g", &[3.0]), [Some(9.0)]);
}

#[test]
fn simplify_refuses_what_is_no_function_text_or_curve() {
    let mut c = build(&[
        "A = (0, 1)",
        "B = (1, 0)",
        "C = (2, 3)",
        "p = Polygon(A, B, C)",
        "l = {1, 2}",
        "t = \"\"",
    ]);
    for argument in ["A", "p", "l", "{1, 2}", "t", "\"\"", "x^3+y^3=1"] {
        rejected(&mut c, &format!("r = Simplify({argument})"));
    }
    rejected(&mut c, "r = Simplify(l, 2)");
}

#[test]
fn a_simplified_text_follows_its_source() {
    let mut c = build(&["a = 2", "t = \"1x+\" + a", "s = Simplify(t)"]);
    assert_eq!(c.text(id(&c, "s")), Some("x + 2"));
    c.set_number(id(&c, "a"), -3.0).unwrap();
    assert_eq!(c.text(id(&c, "s")), Some("x \u{2212} 3"));
    // A text of 4 MB of signs stays within the text bound or is undefined.
    let long = "+-".repeat(1 << 21);
    let mut c = Construction::new();
    script::exec(&mut c, &format!("t = \"{long}\"")).ok();
    if c.id("t").is_some() {
        script::exec(&mut c, "s = Simplify(t)").ok();
    }
}

fn points(c: &Construction, label: &str) -> Option<Vec<(f64, f64)>> {
    let list = c.elements(id(c, label))?;
    Some(
        list.iter()
            .map(|e| match e {
                Element::Value(Kind::Point, Value::Point { x, y }) => (x, y),
                other => panic!("{label} holds {other:?}"),
            })
            .collect(),
    )
}

#[test]
fn plot_solve_puts_the_roots_on_the_x_axis() {
    // CommandsUsingMockedCasTest.cmdPlotSolve
    let c = build(&["r = PlotSolve(x^3-x)"]);
    assert_eq!(
        points(&c, "r").unwrap(),
        [(-1.0, 0.0), (0.0, 0.0), (1.0, 0.0)]
    );
    let mut c = build(&["a = 1", "f(x) = x^2-a", "r = PlotSolve(f)"]);
    assert_eq!(points(&c, "r").unwrap(), [(-1.0, 0.0), (1.0, 0.0)]);
    // No root is an undefined list, not an empty one, and it recovers.
    c.set_number(id(&c, "a"), -1.0).unwrap();
    assert!(points(&c, "r").is_none());
    c.set_number(id(&c, "a"), 4.0).unwrap();
    assert_eq!(points(&c, "r").unwrap(), [(-2.0, 0.0), (2.0, 0.0)]);
}

#[test]
fn plot_solve_of_a_list_or_a_proper_conic_is_undefined() {
    let c = build(&[
        "g: x+y=1",
        "h: x-y=0",
        "k: x^2+y^2=4",
        "r = PlotSolve({g, h})",
        "s = PlotSolve({x^2=1})",
        "t = PlotSolve(k)",
    ]);
    for label in ["r", "s", "t"] {
        assert!(c.elements(id(&c, label)).is_none(), "{label}");
    }
}

#[test]
fn plot_solve_takes_one_equation_and_no_number() {
    let mut c = build(&["f(x) = x^2-3", "a = 2", "A = (1, 2)"]);
    for line in [
        "r = PlotSolve(f, x)",
        "r = PlotSolve(f, x=1)",
        "r = PlotSolve(2)",
        "r = PlotSolve(a)",
        "r = PlotSolve(1=0)",
        "r = PlotSolve()",
        "r = PlotSolve(A)",
    ] {
        rejected(&mut c, line);
    }
}

#[test]
fn equations_typed_in_place_solve_like_named_objects() {
    let c = build(&[
        "r = Solutions(x+y=1, x)",
        "s = Solve(x^2+y^2=4, y)",
        "t = Solutions(x^2=4, x)",
        "u = Solutions(2x+3y=6, y)",
    ]);
    // Main's solving path answers these first; its answers stand.
    assert_eq!(numbers(&c, "t").unwrap(), [-2.0, 2.0]);
}

#[test]
fn extreme_coefficients_end_undefined_or_defined_never_stuck() {
    // Coefficients whose exact products leave the rational range, values
    // that are no numbers, and a conic that degenerates under rounding.
    let mut c = build(&[
        "a = 1",
        "k: 123456789.123456789x^2+987654321.987654321x*y+a*y^2=3.14159265358979",
        "r = Solutions(k)",
        "s = Solve(k, y)",
        "p = PlotSolve(k)",
        "f = Simplify(k)",
    ]);
    for value in [1e300, -1e300, 1e-300, f64::NAN, f64::INFINITY, 0.0, 1.0] {
        c.set_number(id(&c, "a"), value).ok();
        for label in ["r", "s", "p"] {
            if let Some(list) = c.elements(id(&c, label)) {
                assert!(list.len() <= 2, "{label} at a = {value}");
            }
        }
    }
    let mut c = build(&["b = 2", "f(x) = x^9-b*x+1", "r = NSolutions(f, x=b)"]);
    for value in [1e308, -1e308, 1e-320, f64::NAN, f64::NEG_INFINITY, 0.5] {
        c.set_number(id(&c, "b"), value).ok();
        if let Some(roots) = numbers(&c, "r") {
            assert!(roots.len() == 1 && roots[0].is_finite(), "b = {value}");
        }
    }
}

#[test]
fn the_edges_of_the_supported_domains_are_undefined() {
    let c = build(&[
        "f(x) = (x-10^12)^3+(10^36-10^25)",
        "r1 = NSolutions(f, x=0)",
        "g(x) = x^3+x-10^10",
        "r2 = NSolutions(g, x=0)",
        "h(x) = x^3-10^9",
        "r3 = NSolutions(h, x=1)",
    ]);
    // Typed constants past 10^10, a root past 1000.
    for label in ["r1", "r2"] {
        assert!(c.elements(id(&c, label)).is_none(), "{label}");
    }
    assert_eq!(numbers(&c, "r3").unwrap(), [1000.0]);
    // A constructed line is printed by the reference with rounding the
    // kernel does not reproduce.
    let c = build(&[
        "A = (0, 1)",
        "B = (1, 1+1/10^9)",
        "g = Line(A, B)",
        "r = Solutions(g, x)",
    ]);
    assert!(c.elements(id(&c, "r")).is_none());
    // Simplify of a curve whose factored and expanded forms differ.
    let c = build(&[
        "k = Curve((t-10^8)^3/(t-10^8), t, t, 10^8-1, 10^8+1)",
        "s = Simplify(k)",
        "f = Factor(k)",
    ]);
    assert_eq!(curve_point(&c, "s", 1e8 + 1.0), None);
    assert_eq!(curve_point(&c, "f", 1e8 + 1.0), Some((1.0, 1e8 + 1.0)));
}

#[test]
fn the_fifth_round_edges_are_undefined_or_follow_the_reference() {
    // Values that cancel are bounded before the cancellation.
    let mut c = build(&[
        "a = 1",
        "f(x) = a*(a*x+1)-a*(a*x-x)-1",
        "r1 = NSolutions(f, x=0)",
        "r2 = NSolutions(f, {x})",
        "h(x) = (x^3+x-10^9)/(x-1000)",
        "r3 = NSolutions(h, x=0)",
    ]);
    assert_eq!(numbers(&c, "r1").unwrap(), [0.0]);
    assert!(c.elements(id(&c, "r3")).is_none());
    c.set_number(id(&c, "a"), 1e10).unwrap();
    assert!(c.elements(id(&c, "r1")).is_none());
    // A typed line that becomes 0 = 3.
    let mut c = build(&[
        "a = 1",
        "g: a*x=3",
        "r1 = Solutions(g)",
        "r2 = Solve(g)",
        "r3 = Solutions(g, x)",
    ]);
    c.set_number(id(&c, "a"), 0.0).unwrap();
    assert!(c.elements(id(&c, "r1")).is_none());
    assert_eq!(c.elements(id(&c, "r2")).unwrap().len(), 0);
    assert_eq!(c.elements(id(&c, "r3")).unwrap().len(), 0);
    c.set_number(id(&c, "a"), 2.0).unwrap();
    assert_eq!(numbers(&c, "r1").unwrap(), [1.5]);
    // A number object enters with the decimal it was typed with, as on
    // main.
    let c = build(&[
        "a = 0.1234567890123456",
        "f(x) = x+10^10*(a-0.123456789012345)",
        "r = Solutions(f, x)",
    ]);
    let [root] = numbers(&c, "r").unwrap()[..] else {
        panic!("one root");
    };
    assert!((root + 6e-6).abs() < 1e-15);
    // A constructed line with an exactly zero coefficient.
    let c = build(&[
        "A = (0, 1)",
        "B = (0, 2)",
        "g = Line(A, B)",
        "r = Solutions(g)",
    ]);
    assert_eq!(numbers(&c, "r").unwrap(), [0.0]);
    // The sixth round: a list of one variable in the numeric solver answers
    // numbers, also for `y`; a constructed line with whole coefficients is
    // read as printed; ties of the 15 printed digits round up.
    let c = build(&[
        "r = NSolutions(x-1, {x})",
        "e = Element(r, 1)",
        "s = NSolutions(y-1, {y})",
        "A = (0, 0)",
        "B = (1, 1)",
        "g = Line(A, B)",
        "h: x=2",
        "u = NSolutions({g, h})",
        "a = 1.000030517578125",
        "b = 1.000091552734375",
        "f(x) = x+10^8*(b-a-0.00006103515625)",
        "v = Solutions(f, x)",
        "p(x) = (0.1000000001-0.1)*x-0.0000001",
        "w = NSolutions(p, x=0)",
    ]);
    assert_eq!(c.value(id(&c, "e")), Value::Number(1.0));
    assert_eq!(numbers(&c, "s").unwrap(), [1.0]);
    assert_eq!(c.elements(id(&c, "u")).unwrap().len(), 1);
    assert_eq!(numbers(&c, "v").unwrap(), [0.0]);
    assert!(c.elements(id(&c, "w")).is_none());
    // A simplified polynomial curve is evaluated like its printed form.
    let c = build(&[
        "k = Curve(t^2-10^8*t-99999999, t, t, 10^8, 10^8+2)",
        "s = Simplify(k)",
    ]);
    assert_eq!(curve_point(&c, "s", 1e8 + 1.0), Some((1.0, 1e8 + 1.0)));
    // Powers as `Math.pow` computes them.
    let c = build(&[
        "k = Curve(t^5-100020001*t^3+1, t, t, 10000, 10002)",
        "s = Expand(k)",
    ]);
    assert_eq!(curve_point(&c, "s", 10001.0), Some((1.0, 10001.0)));
    // A rational curve coordinate is not simplified.
    let c = build(&[
        "k = Curve((t^2-10^8*t-99999999)/(t-10^8), t, t, 10^8, 10^8+2)",
        "r = Simplify(k)",
    ]);
    assert_eq!(curve_point(&c, "r", 1e8 + 1.0), None);
}
