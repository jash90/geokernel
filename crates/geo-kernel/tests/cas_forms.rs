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
    assert_eq!(numbers(&c, "r2").unwrap(), [1.0]);
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
fn the_second_argument_names_what_it_names() {
    let mut c = build(&["f(x) = x^2-9", "g: x+y=1", "b = 2", "P = (1, 2)"]);
    // `t` is a variable of its own, which the function does not hold.
    for (line, label) in [("r1 = Solutions(f, t)", "r1"), ("r2 = Solve(f, t)", "r2")] {
        script::exec(&mut c, line).unwrap();
        assert_eq!(c.elements(id(&c, label)).unwrap().len(), 0, "{line}");
    }
    for (line, label) in [
        ("r3 = NSolutions(f, t)", "r3"),
        ("r4 = NSolve(f, t)", "r4"),
        ("r5 = Solutions(f, t=1)", "r5"),
    ] {
        script::exec(&mut c, line).unwrap();
        assert!(c.elements(id(&c, label)).is_none(), "{line}");
    }
    // An equation in `t` is solved for it.
    script::exec(&mut c, "r6 = Solutions(t^2=4, t)").unwrap();
    assert_eq!(numbers(&c, "r6").unwrap(), [-2.0, 2.0]);
    script::exec(&mut c, "r7 = Solve(t^2=4, t)").unwrap();
    let lines = c.elements(id(&c, "r7")).unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines
        .iter()
        .all(|e| matches!(e, Element::Value(Kind::Line, Value::Undefined))));
    // A number is printed as its value, which is no variable.
    for (line, label) in [("s1 = Solve(f, b)", "s1"), ("s2 = Solutions(g, b)", "s2")] {
        script::exec(&mut c, line).unwrap();
        assert_eq!(c.elements(id(&c, label)).unwrap().len(), 0, "{line}");
    }
    for (line, label) in [
        ("s3 = NSolutions(f, b)", "s3"),
        ("s4 = Solve(f, b=1)", "s4"),
    ] {
        script::exec(&mut c, line).unwrap();
        assert!(c.elements(id(&c, label)).is_none(), "{line}");
    }
    // `Solve` reads `name = value` as an assumption and solves for x.
    script::exec(&mut c, "u = Solve(f, y=1)").unwrap();
    assert_eq!(c.elements(id(&c, "u")).unwrap().len(), 2);
    // Any other name or object is refused.
    for line in [
        "r = Solutions(f, a)",
        "r = Solve(f, abc)",
        "r = Solutions(a^2=4, a)",
        "r = Solve(f, P)",
        "r = NSolve(f, b)",
        "r = Solutions({x^2=4}, b)",
    ] {
        rejected(&mut c, line);
    }
}

#[test]
fn a_list_of_equations_with_one_variable_gives_one_row() {
    let c = build(&[
        "r1 = Solutions({x^2=4}, x)",
        "r2 = Solutions({2x=4}, x)",
        "r3 = Solutions({x^2=-1}, x)",
        "r4 = Solutions({x^2=4, x=2}, x)",
        "r5 = Solutions({x+y=3, x-y=1}, x)",
        "r6 = NSolutions({x^2=4}, x)",
        "r7 = NSolve({x^2=4}, x)",
        "r8 = NSolutions({x^2=4}, y)",
    ]);
    let rows = c.elements(id(&c, "r1")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(numbers(&c, "r2").unwrap(), [2.0]);
    assert_eq!(c.elements(id(&c, "r3")).unwrap().len(), 0);
    assert_eq!(numbers(&c, "r4").unwrap(), [2.0]);
    assert_eq!(c.elements(id(&c, "r5")).unwrap().len(), 0);
    assert_eq!(c.elements(id(&c, "r6")).unwrap().len(), 0);
    assert!(c.elements(id(&c, "r7")).is_none());
    assert!(c.elements(id(&c, "r8")).is_none());
    // The system solver of the reference does not follow a start value
    // after a list, and no rule gives the root it answers: undefined.
    let c = build(&[
        "s1 = NSolutions({x^2=4}, x=3)",
        "s2 = NSolve({x^2=4}, x=3)",
        "s3 = Solve({x^2=4}, x=3)",
        "s4 = Solutions({x^2=4}, x=3)",
    ]);
    for label in ["s1", "s2", "s3", "s4"] {
        assert!(c.elements(id(&c, label)).is_none(), "{label}");
    }
}

#[test]
fn a_list_of_variables_answers_at_most_one_solution() {
    // The reference lists several solutions in the order of its CAS, which
    // follows no rule; one solution or none is answered.
    let c = build(&[
        "f(x) = x^2-4",
        "g(x) = 2x-4",
        "h(x) = x^2+1",
        "k: x^2+y^2=4",
        "r1 = Solutions(f, {x})",
        "r2 = Solve(f, {x})",
        "r3 = Solutions(k, {y})",
        "r4 = Solutions(g, {x})",
        "r5 = Solve(g, {x})",
        "r6 = Solutions(h, {x})",
    ]);
    for label in ["r1", "r2", "r3"] {
        assert!(c.elements(id(&c, label)).is_none(), "{label}");
    }
    let rows = c.elements(id(&c, "r4")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(c.elements(id(&c, "r5")).unwrap().len(), 1);
    assert_eq!(c.elements(id(&c, "r6")).unwrap().len(), 0);
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
    // The expansion of the numerator has degree 65, past the bound; the
    // reference cancels the pole, so keeping the input would differ.
    let c = build(&[
        "k = Curve((t^65-1)/(t-1), t, t, 0, 2)",
        "e = Expand(k)",
        "j = Curve(sqrt(t)^2, t, t, -1, 1)",
        "s = Simplify(j)",
        "i = Curve(sin(t)/t, t, t, -1, 1)",
        "u = Simplify(i)",
    ]);
    for label in ["e", "s", "u"] {
        assert_eq!(c.value(id(&c, label)), Value::Undefined, "{label}");
        assert_eq!(curve_point(&c, label, 0.5), None, "{label}");
    }
    // A coordinate that is defined everywhere keeps its values.
    let c = build(&[
        "k = Curve(sin(t)^2+t, cos(t)*t^2, t, 0, 2)",
        "e = Factor(k)",
    ]);
    assert_eq!(
        curve_point(&c, "e", 1.0),
        Some((1.0_f64.sin().powi(2) + 1.0, 1.0_f64.cos()))
    );
    // A polynomial past the bound of the expansion is still itself.
    let c = build(&["k = Curve(t^70, t, t, 0, 1)", "e = Expand(k)"]);
    assert_eq!(curve_point(&c, "e", 0.5), Some((0.5_f64.powi(70), 0.5)));
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
    // A start nearby follows the iteration, one too far away for the step
    // budget is undefined; a degree past the bound and a third variable are
    // undefined, not slow.
    let c = build(&[
        "f(x) = x^3-x",
        "r = NSolutions(f, x=1000)",
        "s = NSolutions(f, x=1000000000000)",
        "t = NSolutions(f, x=10^300)",
    ]);
    assert_eq!(numbers(&c, "r").unwrap(), [1.0]);
    assert!(numbers(&c, "s").is_none());
    assert!(numbers(&c, "t").is_none());
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

#[test]
fn equations_typed_in_place_solve_like_named_objects() {
    let c = build(&[
        "r = Solutions(x+y=1, x)",
        "s = Solve(x^2+y^2=4, y)",
        "t = Solutions(x^2=4, x)",
        "u = Solutions(2x+3y=6, y)",
    ]);
    assert_eq!(samples(&c, "r", 0, &[0.0, 3.0]), [1.0, -2.0]);
    assert_eq!(samples(&c, "s", 1, &[0.0]), [2.0]);
    assert_eq!(numbers(&c, "t").unwrap(), [-2.0, 2.0]);
    assert_eq!(samples(&c, "u", 0, &[3.0]), [0.0]);
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
