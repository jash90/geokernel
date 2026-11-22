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
    let c = build(&["l = {1, 10, 0.5}", "a = Sum(l, l)"]);
    assert_eq!(number(&c, "a"), 1.0 * 1.0 + 10.0 * 10.0 + 0.5 * 0.5);
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
fn lists_of_other_lengths_than_the_frequencies_are_undefined() {
    // longer by more than one, shorter: only one more (class bounds) or
    // equal lengths compute (`AlgoStats1D`)
    let c = build(&["b = {1, 2, 3, 4}", "f = {1, 1}", "a = Sum(b, f)"]);
    assert_eq!(c.value(c.id("a").unwrap()), Value::Undefined);
    let d = build(&["b = {1, 2, 3}", "f = {1}", "a = Sum(b, f)"]);
    assert_eq!(d.value(d.id("a").unwrap()), Value::Undefined);
    let e = build(&["b = {1, 2}", "f = {1, 1, 1}", "a = Sum(b, f)"]);
    assert_eq!(e.value(e.id("a").unwrap()), Value::Undefined);
}

#[test]
fn sum_folds_several_arguments() {
    let c = build(&["a = Sum(1, 2, 3)"]);
    assert_eq!(number(&c, "a"), 6.0);
    // points add coordinate-wise, to a point whatever the label
    // (`CmdSum` wraps them in a list of points)
    let p = build(&["s = Sum((1, 2), (3, 4), (5, 6))"]);
    assert_eq!(
        p.value(p.id("s").unwrap()),
        Value::Point { x: 9.0, y: 12.0 }
    );
    assert_eq!(p.kind(p.id("s").unwrap()), geo_kernel::Kind::Point);
    // of two arguments the fold never runs: `CmdSum` wants a list first
    let mut e = Construction::new();
    let error = script::exec(&mut e, "b = Sum(1, 2)").unwrap_err();
    assert!(error.contains("Sum"), "{error}");
    let error = script::exec(&mut e, "b = Sum((1, 2), (3, 4))").unwrap_err();
    assert!(error.contains("Sum"), "{error}");
}

#[test]
fn sum_of_texts_concatenates() {
    let c = build(&[
        "t = Sum({\"Geo\", \"Gebra\"})",
        "u = Sum(\"ab\", \"cd\", \"ef\")",
    ]);
    let joined = ["Geo", "Gebra"].concat();
    assert_eq!(c.text(c.id("t").unwrap()), Some(joined.as_str()));
    assert_eq!(c.text(c.id("u").unwrap()), Some("abcdef"));
    // two texts are not folded either (`CmdSum` `argErr`), nor one alone
    let mut e = Construction::new();
    let error = script::exec(&mut e, "v = Sum(\"ab\", \"cd\")").unwrap_err();
    assert!(error.contains("Sum"), "{error}");
    let error = script::exec(&mut e, "v = Sum(\"ab\")").unwrap_err();
    assert!(error.contains("Sum"), "{error}");
}

#[test]
fn sum_of_functions_splices_their_bodies() {
    // h = x + 2x + x is a function regardless of its label.
    let c = build(&["f(x) = x", "g(x) = 2x", "h = Sum(f, g, f)", "w = h(3)"]);
    assert_eq!(c.kind(c.id("h").unwrap()), geo_kernel::Kind::Function);
    assert_eq!(number(&c, "w"), 12.0);
}
