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
    // Solving for y gives functions, also through `Solve`.
    let c = build(&["k: x^2+y^2=4", "r = Solve(k, y)"]);
    assert_eq!(samples(&c, "r", 0, &[0.0]), [-2.0]);
    assert_eq!(samples(&c, "r", 1, &[0.0]), [2.0]);
}

#[test]
fn the_branches_of_a_conic_show_their_closed_form() {
    let c = build(&["k: x^2+y^2=4", "r = Solutions(k)"]);
    let list = c.elements(id(&c, "r")).unwrap();
    assert_eq!(list.symbolic_at(0).as_deref(), Some("-sqrt(-y^2 + 4)"));
    assert_eq!(list.symbolic_at(1).as_deref(), Some("sqrt(-y^2 + 4)"));
    let c = build(&["k: x^2+x*y+y^2=3", "r = Solutions(k)"]);
    let list = c.elements(id(&c, "r")).unwrap();
    assert_eq!(
        list.symbolic_at(0).as_deref(),
        Some("(-y - sqrt(-3*y^2 + 12))/(2)")
    );
    let c = build(&["k: x*y=1", "r = Solutions(k, y)"]);
    let list = c.elements(id(&c, "r")).unwrap();
    assert_eq!(list.symbolic_at(0).as_deref(), Some("(1)/(x)"));
    // Irrational slopes of a degenerate conic.
    let c = build(&["k: x^2-2y^2=0", "r = Solutions(k)"]);
    let list = c.elements(id(&c, "r")).unwrap();
    assert_eq!(list.symbolic_at(0).as_deref(), Some("(y)*sqrt(2)"));
}

#[test]
fn a_conic_follows_its_parents_through_every_type() {
    let mut c = build(&[
        "a = 4",
        "k: x^2+y^2=a",
        "r = Solutions(k)",
        "n = NSolutions(k)",
    ]);
    assert_eq!(samples(&c, "r", 1, &[0.0]), [2.0]);
    // An empty conic and a single point have no solution, then it recovers.
    for value in [-1.0, 0.0] {
        c.set_number(id(&c, "a"), value).unwrap();
        assert_eq!(c.elements(id(&c, "r")).unwrap().len(), 0);
        assert!(c.elements(id(&c, "n")).is_none());
    }
    c.set_number(id(&c, "a"), 9.0).unwrap();
    assert_eq!(samples(&c, "r", 0, &[0.0]), [-3.0]);
    assert!(samples(&c, "r", 0, &[4.0])[0].is_nan());
}

#[test]
fn an_initial_value_selects_one_root() {
    let mut c = build(&[
        "b = 2",
        "f(x) = x^2-3",
        "r = NSolutions(f, x=b)",
        "s = Solve(f, x=b)",
    ]);
    let root = 3.0_f64.sqrt();
    let single = |c: &Construction| {
        let [value] = numbers(c, "r").expect("a defined list")[..] else {
            panic!("expected one root");
        };
        value
    };
    assert!((single(&c) - root).abs() < 1e-12);
    assert_eq!(c.elements(id(&c, "s")).unwrap().len(), 2);
    c.set_number(id(&c, "b"), -2.0).unwrap();
    assert!((single(&c) + root).abs() < 1e-12);
    // The derivative vanishes at the start: undefined, and defined again.
    c.set_number(id(&c, "b"), 0.0).unwrap();
    assert!(numbers(&c, "r").is_none());
    c.set_number(id(&c, "b"), 1.0).unwrap();
    assert!((single(&c) - root).abs() < 1e-12);
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
    // A start far away halves its way in; a degree past the bound and a
    // third variable are undefined, not slow.
    let c = build(&["f(x) = x^3-x", "r = NSolutions(f, x=1000000000000)"]);
    assert_eq!(numbers(&c, "r").unwrap(), [1.0]);
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
        "k = Curve((t^2-1)/(t-1), sin(t), t, 0, 2)",
        "e = Expand(k)",
        "f = Factor(k)",
        "i = IFactor(k)",
    ]);
    assert_eq!(curve_point(&c, "k", 1.0), None);
    for label in ["e", "f", "i"] {
        assert_eq!(curve_point(&c, label, 1.0), Some((2.0, 1.0_f64.sin())));
        assert_eq!(curve_point(&c, label, 0.5), Some((1.5, 0.5_f64.sin())));
        assert_eq!(c.value(id(&c, label)), c.value(id(&c, "k")));
    }
}

#[test]
fn a_rewritten_curve_follows_its_source() {
    let mut c = build(&[
        "a = 2",
        "k = Curve(a*t^2, cos(a*t), t, 0, a)",
        "e = Expand(k)",
    ]);
    assert_eq!(curve_point(&c, "e", 1.0), Some((2.0, 2.0_f64.cos())));
    // An interval that ends before it starts: undefined with its source.
    c.set_number(id(&c, "a"), -1.0).unwrap();
    assert_eq!(c.value(id(&c, "e")), Value::Undefined);
    assert_eq!(curve_point(&c, "e", 0.0), None);
    c.set_number(id(&c, "a"), 3.0).unwrap();
    assert_eq!(curve_point(&c, "e", 1.0), Some((3.0, 3.0_f64.cos())));
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
