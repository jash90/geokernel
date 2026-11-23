//! Conic records and their readers after moves, rebuilds and rollbacks.

mod common;

use common::*;
use geo_kernel::script::exec;
use geo_kernel::{Construction, Value};

/// Builds `script` fresh and asserts that `c` has the same labels, values
/// and conic records.
fn assert_same_as_fresh(c: &Construction, script: &str, what: &str) {
    let fresh = build(script);
    let labels = |c: &Construction| {
        c.ids()
            .map(|id| c.label(id).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(c), labels(&fresh), "{what}: labels differ");
    if let Some(d) = first_difference(c, &values(c), &values(&fresh)) {
        panic!("{what}: differs from a fresh build: {d}\nfresh script:\n{script}");
    }
}

fn move_to(c: &mut Construction, label: &str, x: f64, y: f64) {
    let id = c.id(label).unwrap();
    c.move_point(id, x, y).unwrap();
}

fn defined(c: &Construction, label: &str) -> bool {
    c.value(c.id(label).unwrap()).is_defined()
}

#[test]
fn a_parabola_whose_directrix_is_undefined_for_a_while() {
    let script = |b: (f64, f64)| {
        format!(
            "F = (0, 1)\nA = (0, -1)\nB = ({}, {})\ng = Line(A, B)\np = Parabola(F, g)\n\
             V = Vertex(p)\n",
            b.0, b.1
        )
    };
    let mut c = build(&script((1.0, -1.0)));
    assert!(defined(&c, "p") && defined(&c, "V"));
    let vertex = c.value(c.id("V").unwrap());
    // With no directrix the parabola remains undefined, but Vertex reads its record.
    move_to(&mut c, "B", 0.0, -1.0);
    for label in ["g", "p"] {
        assert!(!defined(&c, label), "`{label}` is defined with B on A");
    }
    assert_eq!(c.value(c.id("V").unwrap()), vertex);
    let fresh = build(&script((0.0, -1.0)));
    assert!(!defined(&fresh, "V"), "a rebuild has no previous vertex");
    // back: defined again, as if built there
    move_to(&mut c, "B", 1.0, -1.0);
    assert!(defined(&c, "p") && defined(&c, "V"));
    assert_same_as_fresh(&c, &script((1.0, -1.0)), "B back");
}

#[test]
fn the_side_table_after_a_rollback_with_reused_indices() {
    let base = "A = (1, 0)\nB = (0, 1)\nC = (-1, 0)\nD = (0, -1)\nE = (0.6, 0.8)\n\
                k = Conic(A, B, C, D, E)\nM = Center(k)\ng = Line(A, C)\nS = Intersect(g, k, 2)\n\
                r = 2\nm = Dilate(k, r)\n";
    let mut c = build(base);
    move_to(&mut c, "E", 3.0, 0.5);
    move_to(&mut c, "E", 0.6, 0.8);
    assert_same_as_fresh(&c, base, "E away and back");
    let k = c.id("k").unwrap();
    let m = c.id("m").unwrap();
    let (k_before, m_before) = (snap(&c, k), snap(&c, m));
    let at = c.checkpoint();
    exec(&mut c, "n = Ellipse(A, B, C)").unwrap();
    assert!(c.conic(c.id("n").unwrap()).is_some());
    c.rollback(at).unwrap();
    assert!(c.id("n").is_none());
    // new objects at the indices `n` had
    exec(&mut c, "P = (5, 5)").unwrap();
    exec(&mut c, "q = Ellipse(A, B, P)").unwrap();
    assert_eq!(snap(&c, k), k_before, "the record of `k` changed");
    assert_eq!(snap(&c, m), m_before, "the record of `m` changed");
    let full = format!("{base}P = (5, 5)\nq = Ellipse(A, B, P)\n");
    assert_same_as_fresh(&c, &full, "after the rollback");
    let fresh = build(&full);
    assert_eq!(
        snap(&c, c.id("q").unwrap()),
        snap(&fresh, fresh.id("q").unwrap()),
        "the record of `q`"
    );
}

#[test]
fn a_tangent_intersected_with_its_conic_follows_its_point() {
    let script = |q: (f64, f64)| {
        format!(
            "F = (-3, 0)\nG = (3, 0)\nP = (0, 4)\ne = Ellipse(F, G, P)\nQ = ({:?}, {:?})\n\
             t = Tangent(Q, e)\nT = Intersect(t_{{1}}, e)\n",
            q.0, q.1
        )
    };
    let mut c = build(&script((0.0, 6.0)));
    assert!(defined(&c, "T"));
    assert_same_as_fresh(&c, &script((0.0, 6.0)), "at creation");
    let Value::Line { a, b, c: _ } = c.value(c.id("t_{1}").unwrap()) else {
        panic!("the first tangent is not a line");
    };
    let n = a.hypot(b);
    let (dx, dy) = (b / n, -a / n);
    // along the tangent: the tangent and its point stay
    for s in [0.5, -1.25, 2.0] {
        let q = (s * dx, 6.0 + s * dy);
        move_to(&mut c, "Q", q.0, q.1);
        assert!(defined(&c, "T"));
        assert_same_as_fresh(&c, &script(q), &format!("Q moved along the tangent by {s}"));
    }
    // to general positions
    for q in [(2.0, 7.0), (-6.5, 1.5), (0.0, 6.0)] {
        move_to(&mut c, "Q", q.0, q.1);
        assert_same_as_fresh(&c, &script(q), &format!("Q at {q:?}"));
    }
}
