//! Mathematical properties of `Maximize` and `Minimize` over bounded numbers.
//! The Java harness separately records the numerical search results.

use geo_kernel::algo::polygon::PolygonDef;
use geo_kernel::{script, Construction, Def, Value};

fn build(lines: &[&str]) -> Construction {
    let mut c = Construction::new();
    for line in lines {
        script::exec(&mut c, line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
    c
}

fn number(c: &Construction, label: &str) -> Value {
    c.value(c.id(label).unwrap())
}

#[test]
fn maximize_and_minimize_a_quadratic_over_the_slider() {
    let c = build(&[
        "a = Slider(-10, 10)",
        "f = -(a - 3)^2 + 7",
        "m = Maximize(f, a)",
        "n = Minimize(f, a)",
    ]);
    assert_eq!(number(&c, "m"), Value::Number(3.0));
    // The left endpoint is farthest from the parabola's vertex.
    let Value::Number(n) = number(&c, "n") else {
        panic!("the boundary minimum is undefined");
    };
    assert!((-10.0..=-10.0 + 5e-7).contains(&n), "{n}");
}

#[test]
fn maximize_and_minimize_an_inline_expression() {
    let c = build(&[
        "s = Slider(-5, 5)",
        "u = Maximize(5 - (3 - s)^2, s)",
        "v = Minimize(5 + (3 - s)^2, s)",
    ]);
    for label in ["u", "v"] {
        let Value::Number(optimum) = number(&c, label) else {
            panic!("{label} is undefined");
        };
        assert!((optimum - 3.0).abs() < 1e-9, "{label} = {optimum}");
    }
}

#[test]
fn optimize_without_dependency_still_runs_the_search() {
    // Any value in the interval minimizes a constant. Offsetting the
    // objective leaves the choice unchanged.
    let mut c = build(&[
        "a = Slider(-10, 10)",
        "b = 5",
        "g = b",
        "p = Minimize(g, a)",
    ]);
    let Value::Number(optimum) = number(&c, "p") else {
        panic!("the constant objective has no optimum");
    };
    assert!((-10.0..=10.0).contains(&optimum));
    c.set_number(c.id("b").unwrap(), -3.0).unwrap();
    assert_eq!(number(&c, "p"), Value::Number(optimum));
}

#[test]
fn a_locus_keeps_the_current_optima_while_sampling_its_slider() {
    let mut c = build(&[
        "a = Slider(0, 6)",
        "t = Slider(1, 3)",
        "f = (a - t)^2",
        "g = -f",
        "m = Maximize(g, a)",
        "n = Minimize(f, a)",
        "Q = (m + n, t)",
        "L = Locus(Q, t)",
    ]);
    let locus = c.locus(c.id("L").unwrap()).unwrap();
    assert!(locus.defined);
    assert!(locus.points.iter().any(|p| p.y > 2.9));
    assert!(locus.points.iter().all(|p| (p.x - 2.0).abs() < 1e-9));
    assert_eq!(number(&c, "a"), Value::Number(0.0));
    assert_eq!(number(&c, "t"), Value::Number(1.0));
    assert_eq!(number(&c, "f"), Value::Number(1.0));
    assert_eq!(number(&c, "g"), Value::Number(-1.0));
    c.set_number(c.id("t").unwrap(), 2.5).unwrap();
    for label in ["m", "n"] {
        let Value::Number(optimum) = number(&c, label) else {
            panic!("{label} is undefined after updating the slider");
        };
        assert!((optimum - 2.5).abs() < 1e-9, "{label} = {optimum}");
    }
}

#[test]
fn optimize_without_slider_bounds_is_undefined() {
    let c = build(&["c = 2", "h = 3c", "q = Maximize(h, c)"]);
    assert_eq!(number(&c, "q"), Value::Undefined);
    // an expression of an unbounded number is unbounded too
    let d = build(&["c = 2", "q = Maximize(3c, c)"]);
    assert_eq!(number(&d, "q"), Value::Undefined);
}

#[test]
fn an_optimizer_over_its_dependency_budget_stays_undefined() {
    let mut c = build(&["a = Slider(0, 6)", "A = (0, 0)"]);
    let point = c.id("A").unwrap();
    // The area is constant with respect to the slider, so each search trial
    // only needs to recompute the objective.
    for (label, count) in [("u", 5), ("v", 16_385)] {
        c.add_with_list(label, &vec![point; count], |list| {
            Def::Polygon(PolygonDef::AreaPoints(list))
        })
        .unwrap();
        assert_eq!(number(&c, label), Value::Number(0.0));
    }
    for line in [
        "f = u - (a - 3)^2",
        "g = v - (a - 3)^2",
        "m = Maximize(f, a)",
        "n = Maximize(g, a)",
        "b = Circle(A, $arg(Maximize(g, a)))",
    ] {
        script::exec(&mut c, line).unwrap();
    }
    for slider in [0.0, 2.0] {
        c.set_number(c.id("a").unwrap(), slider).unwrap();
        assert_eq!(number(&c, "m"), Value::Number(3.0));
        assert_eq!(number(&c, "n"), Value::Undefined);
        assert_eq!(number(&c, "b"), Value::Undefined);
    }
}

#[test]
fn the_slider_keeps_its_own_value_after_the_search() {
    let mut c = build(&["a = Slider(-10, 10)", "f = a^2", "m = Minimize(f, a)"]);
    assert_eq!(number(&c, "m"), Value::Number(0.0));
    // the search leaves the slider where it was and only the result keeps
    // the optimum
    assert_eq!(number(&c, "a"), Value::Number(0.0));
    c.set_number(c.id("a").unwrap(), 4.0).unwrap();
    assert_eq!(number(&c, "f"), Value::Number(16.0));
    // the optimum follows the relationship, not the slider's position
    assert_eq!(number(&c, "m"), Value::Number(0.0));
}

#[test]
fn optimize_refuses_wrong_arguments() {
    let mut c = build(&["a = Slider(-10, 10)", "f = a^2", "P = (1, 2)"]);
    assert!(script::exec(&mut c, "m = Maximize(f)")
        .unwrap_err()
        .contains("does not take 1"));
    assert!(script::exec(&mut c, "m = Minimize(f, a, a)")
        .unwrap_err()
        .contains("does not take 3"));
    assert!(script::exec(&mut c, "m = Maximize(P, a)")
        .unwrap_err()
        .contains("number"));
}
