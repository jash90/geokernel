//! Loads every golden file, replays each case on `geo-kernel` and compares.

use geo_golden::{gaps, model, repo_root, report};
use geo_kernel::{script, Construction, Def, Error, Kind, Value};

/// Number of cases recorded by `ggb-harness` so far. Raise it when adding cases,
/// so that losing a file is noticed.
const MIN_GOLDEN_CASES: usize = 3670;

#[test]
fn kernel_agrees_with_golden_data() {
    let cases = model::load_all(&repo_root()).unwrap_or_else(|e| panic!("golden data:\n{e}"));
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let r = report::evaluate(&cases, &gaps);
    println!("{}", r.summary());
    assert!(
        r.harness_cases.cmp(&MIN_GOLDEN_CASES).is_ge(),
        "only {} ggb-harness cases, expected at least {MIN_GOLDEN_CASES}",
        r.harness_cases
    );
    assert!(
        r.failures.is_empty(),
        "{}\n{}",
        r.summary(),
        r.failures.join("\n")
    );
}

#[test]
fn axes_are_fixed_before_and_after_editing_a_construction() {
    let mut c = Construction::new();
    let start = c.checkpoint();
    let x = c.id("xAxis").unwrap();
    let y = c.id("yAxis").unwrap();
    assert!(c.is_empty());
    for id in [x, y] {
        assert_eq!(c.kind(id), Kind::Line);
        assert!(!c.def(id).is_free());
        assert!(!c.is_movable(id));
        assert_eq!(
            c.move_point(id, 3.0, 4.0),
            Err(Error::NotFree(c.label(id).to_string()))
        );
    }
    assert!(c.id("XAxis").is_none());
    assert!(c.add("xAxis", Def::Number(1.0)).is_err());
    assert!(c.add("copy", c.def(y)).is_err());
    script::exec(&mut c, "P = Intersect(xAxis, yAxis)").unwrap();
    c.rollback(start).unwrap();
    assert_eq!(c.id("xAxis"), Some(x));
    assert_eq!(c.id("yAxis"), Some(y));
    assert!(c.is_empty());

    let mut w = geo_app_core::workbench::Workbench::default();
    w.run_line("P = Intersect(xAxis, yAxis)").unwrap();
    assert!(!w.delete(w.construction().id("xAxis").unwrap()));
    assert!(w.construction().id("P").is_some());
    assert!(w.delete(w.construction().id("P").unwrap()));
    assert!(w.construction().id("xAxis").is_some());
    assert!(w.undo());
    assert!(w.construction().id("P").is_some());
}

#[test]
fn a_ragged_coefficient_matrix_creates_no_conic() {
    let mut c = script::load("l = {{1,0,0},{0,1},{0,0,-4}}").unwrap();
    for input in ["k = Conic(l)", "k = Conic({{1,0,0},{0,1},{0,0,-4}})"] {
        let err = script::exec(&mut c, input).unwrap_err();
        assert!(err.contains("matrix row"), "{input}: {err}");
        assert!(c.id("k").is_none());
        assert_eq!(c.len(), 1);
    }
}

#[test]
fn interval_commands_still_reject_invalid_arguments() {
    let mut c = script::load("a = 1\nb = 3\nA = (0, 0)").unwrap();
    for cmd in ["Center", "Midpoint"] {
        for arg in ["x > a, x < b", "x = 3", "Midpoint(A, A) < x"] {
            assert!(script::exec(&mut c, &format!("m = {cmd}({arg})")).is_err());
            assert!(c.id("m").is_none());
        }
    }
}

#[test]
fn a_bound_that_reads_x_gives_an_undefined_number() {
    let c = script::load("m = Center(x < x + 1)\nn = Center(x < 2x)").unwrap();
    for label in ["m", "n"] {
        let id = c.id(label).unwrap();
        assert_eq!(c.kind(id), Kind::Number);
        assert_eq!(c.value(id), Value::Undefined);
    }
}

#[test]
fn trilinear_rejects_an_axis_before_packing_its_id() {
    let mut c = script::load("A = (0, 0)\nB = (1, 0)\nC = (0, 1)\na = 1").unwrap();
    for (line, expected) in [
        (
            "P = Trilinear(xAxis, B, C, a, a, a)",
            "`xAxis` is a line, expected a point",
        ),
        (
            "P = Trilinear(A, B, C, a, yAxis, a)",
            "`yAxis` is a line, expected a number",
        ),
    ] {
        assert_eq!(script::exec(&mut c, line).unwrap_err(), expected);
        assert!(c.id("P").is_none());
    }
}

#[test]
fn fitted_svg_view_contains_bounded_conics() {
    for (coefficients, radius) in [
        ("1, 1, 0, 0, 0, 0", 0.0),
        ("1, 1, -4, 0, 0, 0", 2.0),
        // Half-axes 5 and 3 at 45 degrees.
        ("17, 17, -225, -16, 0, 0", 17.0_f64.sqrt()),
    ] {
        let c = script::load(&format!("A = (30, 30)\nk = Conic({coefficients})")).unwrap();
        let view = geo_kernel::svg::View::fit(&c);
        assert!(view.xmin <= -radius && view.ymin <= -radius, "{view:?}");
        assert!(view.xmax >= 30.0 && view.ymax >= 30.0, "{view:?}");
    }
}
