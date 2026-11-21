//! Loads every golden file, replays each case on `geo-kernel` and compares.

use geo_golden::{gaps, model, repo_root, report};
use geo_kernel::{script, Construction, Def, Error, Kind};

/// Number of cases recorded by `ggb-harness` so far. Raise it when adding cases,
/// so that losing a file is noticed.
const MIN_GOLDEN_CASES: usize = 2818;

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
