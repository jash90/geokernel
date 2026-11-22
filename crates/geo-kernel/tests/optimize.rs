//! `Maximize(dep, indep)` and `Minimize(dep, indep)` over a bounded number:
//! the values GeoGebra's `AlgoOptimize` finds with `ExtremumFinder`.

use geo_kernel::{script, Construction, Value};

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
    // the boundary minimum, at the tolerance Brent stops at
    assert_eq!(
        number(&c, "n"),
        Value::Number(-9.999999771177004),
        "{:?}",
        number(&c, "n")
    );
}

#[test]
fn maximize_and_minimize_an_inline_expression() {
    // the forms of GeoGebra's own test (`cmdMinimize`, `cmdMaximize`)
    let c = build(&[
        "s = Slider(-5, 5)",
        "u = Maximize(5 - (3 - s)^2, s)",
        "v = Minimize(5 + (3 - s)^2, s)",
    ]);
    assert_eq!(number(&c, "u"), Value::Number(3.0));
    assert_eq!(number(&c, "v"), Value::Number(3.0000000000000004));
}

#[test]
fn optimize_without_dependency_still_runs_the_search() {
    // a constant dependent value: the finder stops somewhere in the
    // interval, at the same point GeoGebra's does
    let c = build(&[
        "a = Slider(-10, 10)",
        "b = 5",
        "g = b",
        "p = Minimize(g, a)",
    ]);
    assert_eq!(number(&c, "p"), Value::Number(1.055728075070692));
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
    assert!(script::exec(&mut c, "m = Maximize(f, P)")
        .unwrap_err()
        .contains("number"));
}
