//! `ToComplex`, `ToPolar`, `ToPoint`, `DynamicCoordinates`, `Name` and
//! `Object`: the values GeoGebra keeps and the styles the display layer
//! reads.

use geo_kernel::{script, Construction, Value};

fn build(lines: &[&str]) -> Construction {
    let mut c = Construction::new();
    for line in lines {
        script::exec(&mut c, line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
    c
}

fn point(c: &Construction, label: &str) -> (f64, f64) {
    match c.value(c.id(label).unwrap()) {
        Value::Point { x, y } => (x, y),
        v => panic!("{label} is {v:?}"),
    }
}

#[test]
fn conversions_keep_the_value_and_set_the_printing_style() {
    let c = build(&[
        "A = (1, 2)",
        "c1 = ToComplex(A)",
        "p = ToPolar(A)",
        "q = ToPoint(c1)",
        "n = 3",
        "w = ToComplex(n)",
        "l = {5, 2}",
        "u = ToPolar(l)",
    ]);
    // a number becomes (x, 0), a list the point of its first two elements
    assert_eq!(point(&c, "c1"), (1.0, 2.0));
    assert_eq!(point(&c, "p"), (1.0, 2.0));
    assert_eq!(point(&c, "q"), (1.0, 2.0));
    assert_eq!(point(&c, "w"), (3.0, 0.0));
    assert_eq!(point(&c, "u"), (5.0, 2.0));
    use geo_kernel::algo::point::CoordMode;
    assert_eq!(c.coordinate_mode(c.id("c1").unwrap()), Some(CoordMode::Complex));
    assert_eq!(c.coordinate_mode(c.id("p").unwrap()), Some(CoordMode::Polar));
    assert_eq!(c.coordinate_mode(c.id("q").unwrap()), Some(CoordMode::Cartesian));
    // a plain point has no style
    assert_eq!(c.coordinate_mode(c.id("A").unwrap()), None);
}

#[test]
fn a_conversion_follows_its_source() {
    let mut c = build(&["A = (1, 2)", "c1 = ToComplex(A)"]);
    let a = c.id("A").unwrap();
    c.move_point(a, 4.0, -3.0).unwrap();
    assert_eq!(point(&c, "c1"), (4.0, -3.0));
}

#[test]
fn dynamic_coordinates_follow_the_point_through_expressions() {
    let mut c = build(&[
        "P = (1, 1)",
        "M = DynamicCoordinates(P, x(P) + 2, y(P) * 3)",
    ]);
    assert_eq!(point(&c, "M"), (3.0, 3.0));
    let p = c.id("P").unwrap();
    c.move_point(p, 5.0, 2.0).unwrap();
    assert_eq!(point(&c, "M"), (7.0, 6.0));
}

#[test]
fn dynamic_coordinates_refuses_what_it_cannot_move() {
    let mut c = build(&["A = (1, 2)", "C = (3, 4)", "B = Midpoint(A, C)"]);
    let error = script::exec(&mut c, "M = DynamicCoordinates(B, 1, 1)").unwrap_err();
    assert!(error.contains("point") || error.contains("number"), "{error}");
}

#[test]
fn name_gives_the_label_as_text() {
    let c = build(&["txt = \"GeoGebra\"", "a = 5", "n = Name(txt)", "m = Name(a)"]);
    for (label, expected) in [("n", "txt"), ("m", "a")] {
        assert_eq!(c.text(c.id(label).unwrap()), Some(expected));
    }
}

#[test]
fn object_resolves_the_name_at_parse() {
    let c = build(&["a = 5", "b = Object(\"a\")"]);
    assert_eq!(c.value(c.id("b").unwrap()), Value::Number(5.0));
    let mut d = Construction::new();
    script::exec(&mut d, "a = 5").unwrap();
    assert!(script::exec(&mut d, "b = Object(\"c\")")
        .unwrap_err()
        .contains("unknown object `c`"));
}
