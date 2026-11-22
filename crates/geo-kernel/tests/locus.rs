//! `Locus(Q, P)` and `Locus(Q, slider)`: what holds for any sampling. The
//! points themselves are compared with GeoGebra's in `golden/locus`.

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

fn points(c: &Construction, label: &str) -> Vec<(f64, f64, bool)> {
    let l = c.locus(id(c, label)).expect("a locus");
    assert!(l.defined, "`{label}` is undefined");
    l.points.iter().map(|p| (p.x, p.y, p.line_to)).collect()
}

const CIRCLE: [&str; 7] = [
    "A = (1, 1)",
    "r = 2",
    "c = Circle(A, r)",
    "P = Point(c)",
    "B = (5, 0)",
    "Q = Midpoint(P, B)",
    "L = Locus(Q, P)",
];

#[test]
fn the_midpoints_of_a_circle_and_a_point_lie_on_a_circle() {
    let c = build(&CIRCLE);
    let pts = points(&c, "L");
    assert!(pts.len() > 100, "{} points", pts.len());
    // starts and ends at Q, without a line to the start
    let q = c.value(id(&c, "Q"));
    let Value::Point { x, y } = q else {
        panic!("{q:?}")
    };
    assert_eq!((pts[0].0, pts[0].1, pts[0].2), (x, y, false));
    assert_eq!((pts[pts.len() - 1].0, pts[pts.len() - 1].1), (x, y));
    // the circle of radius 1 around the midpoint of A and B
    for &(px, py, _) in &pts {
        assert!(
            ((px - 3.0).hypot(py - 0.5) - 1.0).abs() < 1e-9,
            "({px}, {py})"
        );
    }
    assert_eq!(
        c.value(id(&c, "L")),
        Value::Locus {
            len: pts.len() as u32
        }
    );
}

#[test]
fn sampling_leaves_every_other_object_as_it_was() {
    let without = build(&CIRCLE[..6]);
    let with = build(&CIRCLE);
    for i in without.ids() {
        assert_eq!(
            format!("{:?}", without.value(i)),
            format!("{:?}", with.value(i)),
            "`{}` changed",
            without.label(i)
        );
    }
}

#[test]
fn moving_the_point_on_the_path_keeps_the_locus() {
    let mut c = build(&CIRCLE);
    let before = points(&c, "L");
    c.move_point(id(&c, "P"), 3.0, 1.0).unwrap();
    assert_eq!(points(&c, "L"), before);
    // the start moves with P when an input of the path changes
    c.move_point(id(&c, "B"), -1.0, 0.0).unwrap();
    let after = points(&c, "L");
    assert_ne!(after, before);
    let Value::Point { x, y } = c.value(id(&c, "Q")) else {
        panic!()
    };
    assert_eq!((after[0].0, after[0].1), (x, y));
}

#[test]
fn a_jump_or_an_undefined_stretch_starts_a_new_piece() {
    let c = build(&[
        "A = (-3, 0)",
        "B = (3, 0)",
        "s = Segment(A, B)",
        "P = Point(s)",
        "Q = (x(P), 1 / x(P))",
        "L = Locus(Q, P)",
    ]);
    let pts = points(&c, "L");
    assert!(pts[1..].iter().any(|p| !p.2), "no new piece");
    assert!(pts.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
}

#[test]
fn a_point_on_a_path_between_the_two_points_makes_the_locus_undefined() {
    let c = build(&[
        "A = (1, 1)",
        "r = 2",
        "c = Circle(A, r)",
        "P = Point(c)",
        "B = (5, 0)",
        "k = Line(P, B)",
        "R = Point(k)",
        "Q = Midpoint(R, B)",
        "L = Locus(Q, P)",
    ]);
    assert_eq!(c.value(id(&c, "L")), Value::Undefined);
    assert!(!c.locus(id(&c, "L")).unwrap().defined);
}

#[test]
fn a_locus_needs_a_moving_object_and_a_point_that_depends_on_it() {
    let mut c = build(&CIRCLE[..6]);
    for line in [
        // the point does not depend on P
        "L = Locus(B, P)",
        // neither point is on a path
        "L = Locus(Q, B)",
        // a number that is not a slider
        "a = 2",
        "L = Locus(Q, a)",
    ] {
        let r = script::exec(&mut c, line);
        if line.starts_with("a =") {
            r.unwrap();
        } else {
            assert!(r.is_err(), "`{line}` was accepted");
        }
    }
    // a point on a path does not trace a locus
    let mut c = build(&[
        "A = (0, 0)",
        "B = (4, 0)",
        "s = Segment(A, B)",
        "P = Point(s)",
    ]);
    script::exec(&mut c, "t = Segment(P, B)").unwrap();
    script::exec(&mut c, "Q = Point(t)").unwrap();
    assert!(script::exec(&mut c, "L = Locus(Q, P)").is_err());
}

#[test]
fn sliders_start_at_zero_in_their_interval_and_stay_in_it() {
    let mut c = build(&[
        "a = Slider(1, 4)",
        "b = Slider(-5, -1)",
        "d = Slider(-2, 2, 0.5)",
    ]);
    let v = |c: &Construction, l: &str| c.value(id(c, l));
    assert_eq!(v(&c, "a"), Value::Number(1.0));
    assert_eq!(v(&c, "b"), Value::Number(-1.0));
    assert_eq!(v(&c, "d"), Value::Number(0.0));
    assert_eq!(c.slider_interval(id(&c, "a")), Some((1.0, 4.0)));
    c.set_number(id(&c, "a"), 10.0).unwrap();
    assert_eq!(v(&c, "a"), Value::Number(4.0));
    c.set_number(id(&c, "a"), -3.0).unwrap();
    assert_eq!(v(&c, "a"), Value::Number(1.0));
    assert!(script::exec(&mut c, "e = Slider(1)").is_err());
}

#[test]
fn a_reversed_slider_starts_undefined_and_cannot_drive_a_locus() {
    let mut c = build(&["a = Slider(2, 1)", "Q = (a, 1)", "b = Slider(0, -1)"]);
    let a = id(&c, "a");
    assert_eq!(c.kind(a), Kind::Number);
    assert_eq!(c.value(a), Value::Undefined);
    assert!(script::exec(&mut c, "L = Locus(Q, a)").is_err());
    c.set_number(a, 2.0).unwrap();
    assert_eq!(c.value(a), Value::Number(1.0));
    assert_eq!(c.value(id(&c, "Q")), Value::Point { x: 1.0, y: 1.0 });
    assert!(script::exec(&mut c, "L = Locus(Q, a)").is_err());
    let b = id(&c, "b");
    c.set_number(b, -1.0).unwrap();
    assert_eq!(c.value(b), Value::Number(0.0));
}

#[test]
fn a_slider_locus_runs_through_the_interval_and_follows_free_points() {
    let mut c = build(&[
        "a = Slider(1, 4)",
        "A = (1, 1)",
        "B = (2, 0)",
        "Q = Dilate(B, a, A)",
        "L = Locus(Q, a)",
    ]);
    let pts = points(&c, "L");
    // Q = A + a (B - A) runs from (2, 0) at a = 1 to (5, -3) at a = 4
    assert_eq!((pts[0].0, pts[0].1), (2.0, 0.0));
    for &(x, y, _) in &pts {
        assert!((x - 1.0 + (y - 1.0)).abs() < 1e-9, "({x}, {y})");
        assert!((2.0 - 1e-9..=5.0 + 1e-9).contains(&x), "({x}, {y})");
    }
    // the run stops just before the end of the interval, as upstream's
    // does: a step that would reach it passes the start again
    assert!(pts.iter().any(|p| p.0 > 4.99));
    // setting the slider gives the same points, as it always starts at min
    c.set_number(id(&c, "a"), 3.0).unwrap();
    assert_eq!(points(&c, "L"), pts);
    c.move_point(id(&c, "A"), 0.0, 0.0).unwrap();
    assert_ne!(points(&c, "L"), pts);
}

#[test]
fn a_slider_locus_ignores_a_moved_point_on_a_path() {
    // `Q` reads a point on a path that does not depend on the slider: the
    // locus is recomputed from free objects only
    let mut c = build(&[
        "a = Slider(-2, 2)",
        "A = (2, 0)",
        "r = 2",
        "c = Circle(A, r)",
        "P = Point(c)",
        "Q = (x(P) + a, y(P))",
        "L = Locus(Q, a)",
    ]);
    let before = points(&c, "L");
    c.move_point(id(&c, "P"), 2.0, 2.0).unwrap();
    assert_eq!(points(&c, "L"), before);
    c.set_number(id(&c, "r"), 1.0).unwrap();
    assert_ne!(points(&c, "L"), before);
}

#[test]
fn rolling_back_forgets_loci_and_sliders() {
    let mut c = build(&CIRCLE[..6]);
    let at = c.checkpoint();
    script::exec(&mut c, "L = Locus(Q, P)").unwrap();
    script::exec(&mut c, "a = Slider(1, 4)").unwrap();
    c.rollback(at).unwrap();
    script::exec(&mut c, "a = 7").unwrap();
    let a = id(&c, "a");
    assert_eq!(c.slider_interval(a), None);
    c.set_number(a, 10.0).unwrap();
    assert_eq!(c.value(a), Value::Number(10.0));
    script::exec(&mut c, "L = Locus(Q, P)").unwrap();
    assert!(c.locus(id(&c, "L")).unwrap().defined);
}

#[test]
fn rolling_back_the_first_locus_releases_its_lines_start() {
    let input = [
        "A = (0, 0)",
        "B = (4, 2)",
        "l = Line(A, B)",
        "C = (1, 1)",
        "D = (3, 0)",
        "u = Vector(C, D)",
        "m = Translate(l, u)",
        "P = Point(m)",
        "E = (1, 3)",
        "Q = Midpoint(P, E)",
    ];
    let mut frozen = build(&input);
    script::exec(&mut frozen, "L = Locus(Q, P)").unwrap();
    let mut rolled_back = build(&input);
    let at = rolled_back.checkpoint();
    script::exec(&mut rolled_back, "L = Locus(Q, P)").unwrap();
    rolled_back.rollback(at).unwrap();
    let mut fresh = build(&input);
    for c in [&mut frozen, &mut rolled_back, &mut fresh] {
        c.move_point(id(c, "A"), 1.0, 1.0).unwrap();
    }
    let Value::Point { x, y } = frozen.value(id(&frozen, "P")) else {
        panic!("missing point on the transformed line");
    };
    assert!((x - 0.8).abs() < 1e-9 && (y + 1.6).abs() < 1e-9, "{x}, {y}");
    assert_eq!(
        rolled_back.value(id(&rolled_back, "P")),
        fresh.value(id(&fresh, "P"))
    );
}

const HULL: [&str; 7] = [
    "A = (0, 0)",
    "B = (4, 0)",
    "C = (4, 3)",
    "D = (0, 3)",
    "E = (2, 1)",
    "l = {A, E, B, A, C, D}",
    "h = Hull(l, 1)",
];

#[test]
fn hull_skips_interior_and_duplicate_points_and_closes() {
    let c = build(&HULL);
    // the number argument is only decoration: GeoGebra's `CmdHull` runs
    // `AlgoConvexHull` on the list alone
    let pts = points(&c, "h");
    assert_eq!(
        pts,
        vec![
            (0.0, 0.0, false),
            (0.0, 3.0, true),
            (4.0, 3.0, true),
            (4.0, 0.0, true),
            (0.0, 0.0, true),
        ]
    );
    // the interior point and the duplicate did not reach the hull
    assert_eq!(pts.len(), 5);
}

#[test]
fn hull_follows_a_moved_point() {
    let mut c = build(&HULL);
    c.move_point(id(&c, "E"), -1.0, -1.0).unwrap();
    // E left the square, so the hull runs around it; A became interior
    assert_eq!(
        points(&c, "h"),
        vec![
            (-1.0, -1.0, false),
            (0.0, 3.0, true),
            (4.0, 3.0, true),
            (4.0, 0.0, true),
            (-1.0, -1.0, true),
        ]
    );
}

#[test]
fn hull_of_one_point_is_a_run_of_two() {
    let c = build(&["A = (2, 3)", "l = {A}", "h = Hull(l, 0)"]);
    assert_eq!(points(&c, "h"), vec![(2.0, 3.0, false), (2.0, 3.0, true)]);
}

#[test]
fn hull_of_no_points_and_of_other_objects_is_undefined() {
    let empty = build(&["l = {}", "h = Hull(l, 1)"]);
    assert!(!empty.locus(id(&empty, "h")).unwrap().defined);
    let numbers = build(&["l = {1, 2}", "h = Hull(l, 1)"]);
    assert!(!numbers.locus(id(&numbers, "h")).unwrap().defined);
    let undefined = build(&["a = 1/0", "A = (2, 3)", "l = {A, (a, 0)}", "h = Hull(l, 1)"]);
    // the undefined point is skipped, the hull is the single point
    assert_eq!(
        points(&undefined, "h"),
        vec![(2.0, 3.0, false), (2.0, 3.0, true)]
    );
}

#[test]
fn hull_refuses_wrong_arguments() {
    let mut c = build(&["A = (0, 0)", "l = {A}"]);
    assert!(script::exec(&mut c, "h = Hull(l)")
        .unwrap_err()
        .contains("does not take 1"));
    // the argument resolver reports the wrong kind itself since the
    // calculus merge; both messages refuse the point as the number
    let err = script::exec(&mut c, "h = Hull(l, A)").unwrap_err();
    assert!(
        err.contains("needs a number") || err.contains("expected a number"),
        "{err}"
    );
    let err = script::exec(&mut c, "h = Hull(A, 1)").unwrap_err();
    assert!(
        err.contains("needs a list") || err.contains("expected a list"),
        "{err}"
    );
}

#[test]
fn convex_hull_of_one_list_argument() {
    let c = build(&[
        "l = {(1, 1), (2, 2), (3, 3), (4, 0.25), (5, 0.2)}",
        "h = ConvexHull(l)",
    ]);
    // the collinear run (1,1)..(3,3) keeps only its ends, and (4, 0.25)
    // stays a vertex: it lies below the line from (5, 0.2) back to (1, 1)
    assert_eq!(
        points(&c, "h"),
        vec![
            (1.0, 1.0, false),
            (3.0, 3.0, true),
            (5.0, 0.2, true),
            (4.0, 0.25, true),
            (1.0, 1.0, true),
        ]
    );
}
