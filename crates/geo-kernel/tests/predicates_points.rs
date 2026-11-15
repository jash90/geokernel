//! The predicate commands and the point commands `ClosestPoint` and
//! `Trilinear`: syntax, error messages and degenerate inputs. Values that are
//! compared against GeoGebra live in the golden groups `predicates` and
//! `points`.

use geo_kernel::script::{exec, load};
use geo_kernel::{Construction, Def, Value};

fn value(c: &Construction, label: &str) -> Value {
    c.value(c.id(label).expect("label exists"))
}

const BASE: &str = "rad = 2\nA = (0, 0)\nB = (4, 0)\nC = (0, 3)\nD = (4, 3)\n\
l = Line(A, B)\nm = Line(C, D)\ns = Segment(A, B)\nk = Circle(A, rad)\nr = Ray(A, B)\nE = (2, 0)\nF = (1, 5)\nG = (9, 5)\nH = (-3, 2)\nJ = (0, 7)\nT1 = (2, 0)\nT2 = (2, 5)\nzero = 0\nZ = Circle(A, zero)\n";

fn run(extra: &str) -> Construction {
    load(&format!("{BASE}{extra}")).expect("script loads")
}

#[test]
fn values_do_not_grow() {
    assert!(std::mem::size_of::<Value>() <= 48);
    assert!(std::mem::size_of::<Def>() <= 48);
}

#[test]
fn every_predicate_form_parses() {
    let c = run("ac = Line(A, C)\ncd = Segment(C, D)\nvert = Line(T1, T2)\nt1 = AreCollinear(A, B, E)\nt2 = AreParallel(l, m)\n\
t3 = ArePerpendicular(l, ac)\nt4 = AreConcurrent(l, m, ac)\n\
t5 = AreConcyclic(A, B, C, D)\nt6 = AreEqual(A, A)\nt7 = AreCongruent(s, cd)\n\
t8 = IsTangent(vert, k)\nt9 = AreParallel(s, m)\nt10 = AreParallel(r, m)\n");
    assert_eq!(value(&c, "t1"), Value::Bool(true));
    assert_eq!(value(&c, "t2"), Value::Bool(true));
    assert_eq!(value(&c, "t3"), Value::Bool(true));
    assert_eq!(value(&c, "t4"), Value::Bool(false));
    assert_eq!(value(&c, "t5"), Value::Bool(true));
    assert_eq!(value(&c, "t6"), Value::Bool(true));
    assert_eq!(value(&c, "t7"), Value::Bool(true));
    assert_eq!(value(&c, "t8"), Value::Bool(true));
    assert_eq!(value(&c, "t9"), Value::Bool(true));
    assert_eq!(value(&c, "t10"), Value::Bool(true));
}

#[test]
fn predicate_argument_errors() {
    let mut c = run("");
    for bad in [
        "t = AreCollinear(A, B)",
        "t = AreCollinear(A, B, l)",
        "t = AreParallel(l)",
        "t = AreParallel(A, B)",
        "t = ArePerpendicular(l, k)",
        "t = AreConcurrent(l, m)",
        "t = AreConcurrent(A, B, C)",
        "t = AreConcyclic(A, B, C)",
        "t = AreEqual(A)",
        "t = AreCongruent(A, B, C)",
        "t = IsTangent(k, l)",
        "t = IsTangent(A, k)",
        "t = AreCollinear(A, B, nope)",
    ] {
        assert!(exec(&mut c, bad).is_err(), "{bad} must fail");
    }
}

#[test]
fn degenerate_predicates_do_not_panic() {
    let c = run(
        "t1 = AreCollinear(A, A, B)\nt2 = AreConcyclic(A, A, B, B)\n\
t3 = AreEqual(l, l)\nt4 = AreParallel(l, l)\nt5 = IsTangent(l, Z)\n",
    );
    assert_eq!(value(&c, "t1"), Value::Bool(true));
    assert_eq!(value(&c, "t3"), Value::Bool(true));
    for label in ["t2", "t4", "t5"] {
        assert!(matches!(value(&c, label), Value::Bool(_)), "{label}");
    }
}

#[test]
fn closest_point_forms() {
    let c = run("p1 = ClosestPoint(l, F)\np2 = ClosestPoint(F, l)\n\
p3 = ClosestPoint(s, G)\np4 = ClosestPoint(r, H)\np5 = ClosestPoint(k, J)\n\
p6 = ClosestPoint(k, A)\n");
    assert_eq!(value(&c, "p1"), Value::Point { x: 1.0, y: 0.0 });
    assert_eq!(value(&c, "p2"), Value::Point { x: 1.0, y: 0.0 });
    assert_eq!(value(&c, "p3"), Value::Point { x: 4.0, y: 0.0 });
    assert_eq!(value(&c, "p4"), Value::Point { x: 0.0, y: 0.0 });
    match value(&c, "p5") {
        Value::Point { x, y } => assert!(x.abs() < 1e-12 && (y - 2.0).abs() < 1e-12),
        other => panic!("{other:?}"),
    }
}

#[test]
fn closest_point_errors() {
    let mut c = run("");
    for bad in [
        "p = ClosestPoint(l)",
        "p = ClosestPoint(l, m)",
        "p = ClosestPoint(A, B)",
        "p = ClosestPoint(l, 3)",
    ] {
        assert!(exec(&mut c, bad).is_err(), "{bad} must fail");
    }
}

#[test]
fn trilinear_forms() {
    let mut c = run("n1 = 1\nnm = -1\nn0 = 0\n");
    // Equal trilinear coordinates give the incentre of the 3-4-5 triangle.
    exec(&mut c, "I = Trilinear(A, B, C, n1, n1, n1)").unwrap();
    match value(&c, "I") {
        Value::Point { x, y } => {
            assert!(
                (x - 1.0).abs() < 1e-12 && (y - 1.0).abs() < 1e-12,
                "{x} {y}"
            );
        }
        other => panic!("{other:?}"),
    }
    for bad in [
        "Q = Trilinear(A, B, C, n1, n1)",
        "Q = Trilinear(A, B, l, n1, n1, n1)",
        "Q = Trilinear(A, B, C, A, n1, n1)",
    ] {
        assert!(exec(&mut c, bad).is_err(), "{bad} must fail");
    }
    // Weights summing to 0 give a point at infinity, which has no value.
    exec(&mut c, "K = Trilinear(A, B, C, n1, nm, n0)").unwrap();
    assert!(matches!(
        value(&c, "K"),
        Value::Point { .. } | Value::Undefined
    ));
}
