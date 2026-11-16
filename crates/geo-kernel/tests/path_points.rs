//! Points on paths (`Point(path)`, `Point(path, u)`, `PathParameter(P)`):
//! the two ways their state can go wrong. A point must be recomputed when an
//! input that its path does not show in its value changes (the start point of
//! a line, a vertex of a polygon), and the side table of states must stay
//! right through rollbacks that reuse indices. Each construction is compared,
//! bit for bit, with one built fresh at the final positions whose points on
//! paths get the states of the original (`Construction::restore_path_state`).

mod common;

use common::*;
use geo_kernel::script::exec;
use geo_kernel::{Construction, Error, Value};

/// Builds `script` fresh, gives its points on paths the states they have in
/// `c`, and asserts that every label, value and state is the same.
fn assert_same_as_fresh(c: &Construction, script: &str, what: &str) {
    let mut fresh = build(script);
    let labels = |c: &Construction| {
        c.ids()
            .map(|id| c.label(id).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(c), labels(&fresh), "{what}: labels differ");
    for id in c.ids() {
        if let Some(state) = c.path_state(id) {
            let target = fresh.id(c.label(id)).unwrap();
            fresh.restore_path_state(target, state).unwrap();
        }
    }
    if let Some(d) = first_difference(c, &values(c), &values(&fresh)) {
        panic!("{what}: differs from a fresh build: {d}\nfresh script:\n{script}");
    }
}

fn move_to(c: &mut Construction, label: &str, x: f64, y: f64) -> usize {
    let id = c.id(label).unwrap();
    c.move_point(id, x, y).unwrap()
}

fn point(c: &Construction, label: &str) -> (f64, f64) {
    match c.value(c.id(label).unwrap()) {
        Value::Point { x, y } => (x, y),
        v => panic!("`{label}` is {v:?}"),
    }
}

#[test]
fn a_start_point_moving_along_its_line_moves_the_point() {
    // `h` is the perpendicular through C to a horizontal line: moving C up
    // leaves `h` the same, bit for bit, but C is the start point the point
    // on `h` is measured from (`AlgoOrthoLinePointLine`)
    let script = |cy: f64| {
        format!(
            "A = (0, 0)\nB = (4, 0)\ng = Line(A, B)\nC = (1, {cy:?})\nh = PerpendicularLine(C, g)\n\
             u = 0.3\nU = Point(h, u)\nP = Point(h)\nt = PathParameter(P)\n"
        )
    };
    let mut c = build(&script(2.0));
    move_to(&mut c, "P", 3.0, 5.0);
    let h = c.id("h").unwrap();
    let line_before = c.value(h);
    let (u_before, p_before) = (point(&c, "U"), point(&c, "P"));
    move_to(&mut c, "C", 1.0, 4.0);
    assert_eq!(
        c.value(h),
        line_before,
        "the line was meant to stay the same"
    );
    assert_eq!(
        point(&c, "U").1,
        u_before.1 + 2.0,
        "U did not follow its start point"
    );
    assert_eq!(
        point(&c, "P").1,
        p_before.1 + 2.0,
        "P did not follow its start point"
    );
    assert_same_as_fresh(&c, &script(4.0), "C moved along h");
}

#[test]
fn a_vertex_moving_parallel_to_the_opposite_side_moves_the_point() {
    // the area of the triangle stays, so its value stays
    let script = |cx: f64| {
        format!(
            "A = (0, 0)\nB = (4, 0)\nC = ({cx:?}, 3)\nq = Polygon(A, B, C)\nu = 0.6\n\
             U = Point(q, u)\nP = Point(q)\nt = PathParameter(P)\nl = PolyLine(A, B, C)\nQ = Point(l)\n"
        )
    };
    let mut c = build(&script(1.0));
    move_to(&mut c, "P", 1.0, 2.5);
    move_to(&mut c, "Q", 3.5, 2.0);
    let q = c.id("q").unwrap();
    let area = c.value(q);
    let (u_before, p_before, q_before) = (point(&c, "U"), point(&c, "P"), point(&c, "Q"));
    move_to(&mut c, "C", 2.0, 3.0);
    assert_eq!(c.value(q), area, "the area was meant to stay the same");
    assert_ne!(point(&c, "U"), u_before, "U did not follow the vertex");
    assert_ne!(point(&c, "P"), p_before, "P did not follow the vertex");
    assert_ne!(point(&c, "Q"), q_before, "Q did not follow the vertex");
    assert_same_as_fresh(&c, &script(2.0), "C moved parallel to AB");
}

#[test]
fn the_side_table_after_a_rollback_with_reused_indices() {
    let base = "A = (0, 0)\nB = (4, 2)\ns = Segment(A, B)\nP = Point(s)\nM = (1, 1)\nr = 2\n\
                k = Circle(M, r)\nu = 0.25\nQ = Point(k, u)\nt = PathParameter(P)\n";
    let mut c = build(base);
    move_to(&mut c, "P", 3.0, 0.0);
    move_to(&mut c, "B", 6.0, 0.0);
    let p = c.id("P").unwrap();
    let q = c.id("Q").unwrap();
    let (p_state, q_state) = (c.path_state(p).unwrap(), c.path_state(q).unwrap());
    let at = c.checkpoint();
    // two points on paths after the checkpoint, so that a state left behind
    // would sit before the new one and the table would be out of order
    exec(&mut c, "R = Point(k)").unwrap();
    exec(&mut c, "V = Point(s)").unwrap();
    exec(&mut c, "w = PathParameter(R)").unwrap();
    let r = c.id("R").unwrap();
    move_to(&mut c, "R", -5.0, 1.0);
    move_to(&mut c, "V", 1.0, 1.0);
    c.rollback(at).unwrap();
    assert!(c.id("R").is_none() && c.path_state(r).is_none());
    // new objects at the indices `R` and `V` had: a point on another path
    // and an object without a state
    exec(&mut c, "S = Point(s)").unwrap();
    exec(&mut c, "w = PathParameter(S)").unwrap();
    assert_eq!(c.id("S"), Some(r), "the index was meant to be reused");
    assert_eq!(c.path_state(p), Some(p_state), "the state of P changed");
    assert_eq!(c.path_state(q), Some(q_state), "the state of Q changed");
    let full = |bx: f64| {
        format!(
            "A = (0, 0)\nB = ({bx:?}, 0)\ns = Segment(A, B)\nP = Point(s)\nM = (1, 1)\nr = 2\n\
             k = Circle(M, r)\nu = 0.25\nQ = Point(k, u)\nt = PathParameter(P)\nS = Point(s)\n\
             w = PathParameter(S)\n"
        )
    };
    // `S` is new: its state is that of a fresh build, without restoring
    let fresh = build(&full(6.0));
    assert_eq!(c.path_state(r), fresh.path_state(fresh.id("S").unwrap()));
    assert_same_as_fresh(&c, &full(6.0), "after the rollback");
    // and the reused slot follows its own path from now on
    move_to(&mut c, "B", 8.0, 0.0);
    assert_same_as_fresh(&c, &full(8.0), "after moving B");
}

#[test]
fn a_discarded_command_leaves_no_state_behind() {
    let mut c = build("A = (0, 0)\nB = (4, 2)\ng = Line(A, B)\nP = Point(g)\n");
    let len = c.len();
    // a failing line is all or nothing
    assert!(exec(&mut c, "Q = Point(g, A)").is_err());
    assert!(exec(&mut c, "Q = Point(A)").is_err());
    assert!(exec(&mut c, "Q = PathParameter(g)").is_err());
    assert_eq!(c.len(), len);
    exec(&mut c, "Q = Point(g)").unwrap();
    let q = c.id("Q").unwrap();
    assert!(c.path_state(q).is_some());
}

#[test]
fn only_a_point_made_by_point_of_a_path_moves() {
    let mut c = build(
        "A = (0, 0)\nB = (4, 2)\ng = Line(A, B)\nP = Point(g)\nu = 0.5\nU = Point(g, u)\n\
         t = PathParameter(P)\n",
    );
    let id = |c: &Construction, l: &str| c.id(l).unwrap();
    assert!(c.is_movable(id(&c, "A")) && c.is_movable(id(&c, "P")));
    assert!(!c.is_movable(id(&c, "U")) && !c.is_movable(id(&c, "g")));
    assert!(!c.is_movable(id(&c, "t")));
    let (u, g, t) = (id(&c, "U"), id(&c, "g"), id(&c, "t"));
    assert!(matches!(c.move_point(u, 1.0, 1.0), Err(Error::NotFree(_))));
    assert!(matches!(c.move_point(g, 1.0, 1.0), Err(Error::NotFree(_))));
    let p = id(&c, "P");
    assert_eq!(c.move_point(p, f64::NAN, 1.0), Err(Error::NotFinite));
    // the state of one point cannot be given to an object without one
    let state = c.path_state(p).unwrap();
    assert!(matches!(
        c.restore_path_state(t, state),
        Err(Error::NotFree(_))
    ));
    // a move that projects to the same state recomputes nothing
    let n = move_to(&mut c, "P", 3.0, 5.0);
    assert!(n > 0);
    assert_eq!(move_to(&mut c, "P", 3.0, 5.0), 0);
    // the projection: (3, 5) onto the line through (0, 0) and (4, 2)
    let (x, y) = point(&c, "P");
    assert!(
        (x - 4.4).abs() < 1e-12 && (y - 2.2).abs() < 1e-12,
        "P = ({x}, {y})"
    );
}

#[test]
fn a_point_keeps_its_parameter_through_an_undefined_path() {
    let script = "A = (0, 0)\nB = (4, 2)\ns = Segment(A, B)\nP = Point(s)\nt = PathParameter(P)\n";
    let mut c = build(script);
    move_to(&mut c, "P", 3.0, 1.0);
    let t = c.id("t").unwrap();
    let before = c.value(t);
    // A on B: length 0, the point sits on A and keeps its parameter
    move_to(&mut c, "A", 4.0, 2.0);
    assert_eq!(point(&c, "P"), (4.0, 2.0));
    move_to(&mut c, "A", 0.0, 0.0);
    assert_eq!(c.value(t), before);
    // an undefined line: the point is undefined and comes back
    let mut c =
        build("A = (0, 0)\nB = (4, 2)\ng = Line(A, B)\nP = Point(g)\nt = PathParameter(P)\n");
    move_to(&mut c, "P", 3.0, 5.0);
    let t = c.id("t").unwrap();
    let before = c.value(t);
    move_to(&mut c, "B", 0.0, 0.0);
    assert!(!c.value(c.id("P").unwrap()).is_defined());
    assert!(!c.value(t).is_defined());
    move_to(&mut c, "B", 4.0, 2.0);
    assert_eq!(c.value(t), before);
}

#[test]
fn points_follow_an_arc_that_moves_at_the_same_length() {
    // the arc's value, its length, stays the same bit for bit while its
    // centre moves: the points on it must still follow the record
    let script = |mx: f64, my: f64| {
        format!(
            "M = ({mx:?}, {my:?})\nA = M + (3, 0)\nB = M + (0, 3)\na = CircularArc(M, A, B)\n\
             s = CircumcircularSector(A, M, B)\nu = 0.4\nU = Point(a, u)\nP = Point(a)\n\
             t = PathParameter(P)\nQ = Point(s)\nm = Midpoint(P, U)\n"
        )
    };
    let mut c = build(&script(0.0, 0.0));
    move_to(&mut c, "P", 1.0, 5.0);
    move_to(&mut c, "Q", -4.0, 4.0);
    let a = c.id("a").unwrap();
    let length = c.value(a);
    let before = [
        point(&c, "U"),
        point(&c, "P"),
        point(&c, "Q"),
        point(&c, "m"),
    ];
    move_to(&mut c, "M", 2.0, -1.0);
    assert_eq!(c.value(a), length, "the length was meant to stay the same");
    let after = [
        point(&c, "U"),
        point(&c, "P"),
        point(&c, "Q"),
        point(&c, "m"),
    ];
    for (b, a) in before.iter().zip(&after) {
        assert_ne!(b, a, "a point did not follow the arc");
    }
    assert_same_as_fresh(&c, &script(2.0, -1.0), "the arc moved");
}

#[test]
fn the_arc_table_after_a_rollback_with_reused_indices() {
    let base = "M = (0, 0)\nA = (3, 0)\nB = (0, 3)\na = CircularArc(M, A, B)\nP = Point(a)\n";
    let mut c = build(base);
    move_to(&mut c, "P", 1.0, 2.0);
    let a = c.id("a").unwrap();
    // `Debug` shows every field, with the sign of zero
    let bits = |r: Option<geo_kernel::ArcData>| format!("{r:?}");
    let record = bits(c.arc(a));
    let at = c.checkpoint();
    // two parts after the checkpoint, so that a record left behind would
    // sit before the new one and the table would be out of order
    exec(&mut c, "b = CircumcircularSector(A, B, M)").unwrap();
    exec(&mut c, "d = Semicircle(A, B)").unwrap();
    exec(&mut c, "R = Point(d)").unwrap();
    let b = c.id("b").unwrap();
    let d = c.id("d").unwrap();
    // only objects added since the checkpoint may move before a rollback
    move_to(&mut c, "R", 5.0, 1.0);
    c.rollback(at).unwrap();
    assert!(c.arc(b).is_none() && c.arc(d).is_none());
    assert_eq!(bits(c.arc(a)), record, "the record of a changed");
    // new objects at the indices `b` and `d` had: a part on other points,
    // whose record a left-behind one would come after, and one without a
    // record
    exec(&mut c, "e = Semicircle(B, M)").unwrap();
    exec(&mut c, "n = 2").unwrap();
    exec(&mut c, "Q = Point(e)").unwrap();
    assert_eq!(c.id("e"), Some(b), "the index was meant to be reused");
    assert_eq!(c.id("n"), Some(d), "the index was meant to be reused");
    assert!(c.arc(d).is_none(), "a number has a part record");
    let full = |ax: f64| {
        format!(
            "M = (0, 0)\nA = ({ax:?}, 0)\nB = (0, 3)\na = CircularArc(M, A, B)\nP = Point(a)\n\
             e = Semicircle(B, M)\nn = 2\nQ = Point(e)\n"
        )
    };
    assert_same_as_fresh(&c, &full(3.0), "after the rollback");
    let fresh = build(&full(3.0));
    assert_eq!(bits(c.arc(b)), bits(fresh.arc(fresh.id("e").unwrap())));
    // and the reused slot follows its own points from now on
    move_to(&mut c, "A", 4.0, 0.0);
    move_to(&mut c, "M", 0.0, -1.0);
    let full = full(4.0).replace("M = (0, 0)", "M = (0.0, -1.0)");
    assert_same_as_fresh(&c, &full, "after moving A and M");
}
