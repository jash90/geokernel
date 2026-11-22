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
