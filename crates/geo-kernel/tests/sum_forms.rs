//! The remaining forms of `Sum`: frequencies, class bounds, and the fold
//! of several arguments (`CmdSum`).

use geo_kernel::{script, Construction, Value};

fn build(lines: &[&str]) -> Construction {
    let mut c = Construction::new();
    for line in lines {
        script::exec(&mut c, line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
    c
}

fn number(c: &Construction, label: &str) -> f64 {
    match c.value(c.id(label).unwrap()) {
        Value::Number(v) => v,
        v => panic!("{label} is {v:?}"),
    }
}

#[test]
fn sum_with_frequencies_weights_the_elements() {
    // GeoGebra's own test: Sum[{1,10,1/2}, {1,10,1/2}] = 101.25
    let c = build(&["l = {1, 10, 0.5}", "a = Sum(l, l)"]);
    assert_eq!(number(&c, "a"), 101.25);
}

#[test]
fn sum_with_class_bounds_uses_their_midpoints() {
    // one more element than frequencies: (1+2)/2 and (2+3)/2, once each
    let c = build(&["b = {1, 2, 3}", "f = {1, 1}", "a = Sum(b, f)"]);
    assert_eq!(number(&c, "a"), 4.0);
    // a class list without frequencies stays the plain sum
    let d = build(&["b = {1, 2, 3}", "a = Sum(b)"]);
    assert_eq!(number(&d, "a"), 6.0);
}

#[test]
fn a_negative_frequency_or_a_missing_element_is_undefined() {
    let c = build(&["b = {1, 2}", "f = {1, -1}", "a = Sum(b, f)"]);
    assert_eq!(c.value(c.id("a").unwrap()), Value::Undefined);
    // three arguments of a list are not the frequencies form
    let mut d = build(&["b = {1, 2}", "f = {1}"]);
    let error = script::exec(&mut d, "a = Sum(b, f, 1)").unwrap_err();
    assert!(error.contains("does not take 3"), "{error}");
}

#[test]
fn sum_folds_several_arguments() {
    let c = build(&["a = Sum(1, 2, 3)", "b = Sum(1, 2)"]);
    assert_eq!(number(&c, "a"), 6.0);
    assert_eq!(number(&c, "b"), 3.0);
    // points add coordinate-wise
    let p = build(&["s = Sum((1, 2), (3, 4), (5, 6))"]);
    assert_eq!(
        p.value(p.id("s").unwrap()),
        Value::Vector {
            x: 9.0,
            y: 12.0
        }
    );
}

#[test]
fn sum_of_texts_concatenates() {
    // GeoGebra's own test: Sum[{"Geo","Gebra"}] = GeoGebra
    let c = build(&["t = Sum({\"Geo\", \"Gebra\"})", "u = Sum(\"ab\", \"cd\")"]);
    assert_eq!(c.text(c.id("t").unwrap()), Some("GeoGebra"));
    assert_eq!(c.text(c.id("u").unwrap()), Some("abcd"));
}
