//! The explicit output labels `{a, b} = Command(...)` and the forced forms
//! `$point(x, y)` and `$vector(x, y)` of the script language: what they do,
//! that existing lines keep their meaning, and that damaged lines of these
//! forms fail without changing the construction or panicking.

mod common;

use common::Rng;
use geo_kernel::script::{exec, load};
use geo_kernel::{Construction, Kind, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

const BASE: &str = "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\nB = (4, 0)\nC = (0, 1)\n\
                    l = Line(A, B)\nm = Line(A, C)\n";

fn labels(c: &Construction) -> Vec<String> {
    c.ids().map(|id| c.label(id).to_string()).collect()
}

#[test]
fn explicit_labels_replace_the_rule_of_the_kernel() {
    // The same command, labelled by the rule and by the line.
    let by_rule = load(&format!("{BASE}t = Tangent(P, k)\n")).unwrap();
    let explicit = load(&format!("{BASE}{{t_{{1}}, t_{{2}}}} = Tangent(P, k)\n")).unwrap();
    for label in ["t_{1}", "t_{2}"] {
        assert_eq!(
            by_rule.value(by_rule.id(label).unwrap()),
            explicit.value(explicit.id(label).unwrap())
        );
    }
    assert_eq!(labels(&by_rule), labels(&explicit));
    // any names, in the order of GeoGebra's outputs
    let c = load(&format!("{BASE}{{first, second}} = Tangent(P, k)\n")).unwrap();
    let (first, second) = (
        c.value(c.id("first").unwrap()),
        c.value(c.id("second").unwrap()),
    );
    assert_ne!(first, second);
    assert_eq!(first, explicit.value(explicit.id("t_{1}").unwrap()));
    // a slot left empty creates nothing
    let c = load(&format!("{BASE}{{, second}} = Tangent(P, k)\n")).unwrap();
    assert!(c.id("second").is_some());
    assert_eq!(c.len(), load(BASE).unwrap().len() + 1);
}

#[test]
fn explicit_labels_do_not_depend_on_what_is_defined() {
    // Tangents from the centre are undefined; the rule would create one object.
    let rule = load(&format!("{BASE}t = Tangent(A, k)\n")).unwrap();
    assert_eq!(rule.len(), load(BASE).unwrap().len() + 1);
    let c = load(&format!("{BASE}{{u, v}} = Tangent(A, k)\n")).unwrap();
    assert_eq!(c.len(), load(BASE).unwrap().len() + 2);
    assert_eq!(c.value(c.id("u").unwrap()), Value::Undefined);
    // and they keep following their parents
    let mut c = c;
    let a = c.id("A").unwrap();
    c.move_point(a, 1.0, 1.0).unwrap();
    assert_eq!(c.kind(c.id("v").unwrap()), Kind::Line);
}

#[test]
fn existing_lines_mean_what_they_did() {
    for line in [
        "t = Tangent(P, k)",
        "b = AngularBisector(l, m)",
        "w_7 = AngularBisector(l, m)",
        "X = Intersect(l, k, 2)",
        "u = (1, 2)",
        "Q = (1, 2)",
    ] {
        let mut c = load(BASE).unwrap();
        exec(&mut c, line).unwrap();
    }
    // a lower-case free point and an upper-case free vector come only from the new forms
    let c = load("u = (1, 2)\nQ = (1, 2)\n").unwrap();
    assert_eq!(c.kind(c.id("u").unwrap()), Kind::Vector);
    assert_eq!(c.kind(c.id("Q").unwrap()), Kind::Point);
}

#[test]
fn forced_forms_and_indexed_labels() {
    let c =
        load("p = $point(1.5, -2)\nV = $vector(3, 4)\nt_{1} = (5, 6)\nT_{12} = (1, 1)\n").unwrap();
    assert_eq!(
        c.value(c.id("p").unwrap()),
        Value::Point { x: 1.5, y: -2.0 }
    );
    assert_eq!(
        c.value(c.id("V").unwrap()),
        Value::Vector { x: 3.0, y: 4.0 }
    );
    assert_eq!(c.kind(c.id("t_{1}").unwrap()), Kind::Vector);
    assert_eq!(c.kind(c.id("T_{12}").unwrap()), Kind::Point);
    let c = load("v_{max} = (1, 2)\nP_{ab'} = (1, 1)\n").unwrap();
    assert!(c.id("v_{max}").is_some() && c.id("P_{ab'}").is_some());
    let c = load("{u_{a}, w_{b}} = Tangent(P, k)\n");
    assert!(c.is_err()); // P and k do not exist here
    let c = load(&format!(
        "{BASE}{{u_{{a}}, w_{{b}}}} = Tangent(P, k)\nV_{{max}} = Midpoint(A, B)\n"
    ))
    .unwrap();
    assert!(c.id("u_{a}").is_some() && c.id("V_{max}").is_some());
    // a lower-case point is usable in constructions
    let c =
        load("p = $point(0, 0)\nq = $point(4, 0)\nM = Midpoint(p, q)\nl = Line(p, q)\n").unwrap();
    assert_eq!(c.value(c.id("M").unwrap()), Value::Point { x: 2.0, y: 0.0 });
    for bad in [
        "t_{ = (1, 1)",
        "t_{} = (1, 1)",
        "t_{a,b} = (1, 1)",
        "t_{a b} = (1, 1)",
        "_{1} = (1, 1)",
        "t_{1}x = (1, 1)",
    ] {
        let mut c = Construction::new();
        assert!(exec(&mut c, bad).is_err(), "{bad}");
        assert!(c.is_empty(), "{bad}");
    }
}

/// Damaged lines of the new forms: an error, no panic, nothing created.
#[test]
fn damaged_lines_fail_cleanly() {
    let pieces = [
        "{", "}", "{{", "}}", ",", ", ,", "t_{1}", "t_{2}", "u", "k", "=", "==", "Tangent",
        "(P, k)", "(", ")", "$point", "$vector", "(1, 2)", "1", "x", " ", "_{", "#", "'", "\u{b7}",
        "A",
    ];
    let mut rng = Rng::new(0x5eed);
    let mut c = load(BASE).unwrap();
    let mut created = 0;
    for _ in 0..20_000 {
        let n = 1 + rng.below(9);
        let line: String = (0..n).map(|_| *rng.pick(&pieces)).collect();
        let before = labels(&c);
        let result = catch_unwind(AssertUnwindSafe(|| exec(&mut c, &line)))
            .unwrap_or_else(|_| panic!("panic on {line:?}"));
        match result {
            Ok(Some(_)) => created += 1,
            Ok(None) => {}
            Err(_) => assert_eq!(labels(&c), before, "{line:?} changed the construction"),
        }
        if c.len() > 400 {
            c = load(BASE).unwrap();
        }
    }
    println!("{created} of 20000 damaged lines created objects");
}
