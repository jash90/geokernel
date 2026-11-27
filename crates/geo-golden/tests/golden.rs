//! Loads every golden file, replays each case on `geo-kernel` and compares.

use geo_golden::{gaps, model, repo_root, report};
use geo_kernel::{script, Construction, Def, Error, Kind, Value};

/// Number of cases recorded by `ggb-harness` so far. Raise it when adding cases,
/// so that losing a file is noticed.
const MIN_GOLDEN_CASES: usize = 7606;

#[test]
fn fit_lines_match_raw_geogebra_coefficients() {
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| case.id.starts_with("statistics-fitline"));
    assert_eq!(cases.len(), 11);
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
    assert!(report.scale_agree > 0);
    assert_eq!(report.scale_disagree, 0, "{}", report.summary());
}

#[test]
fn paired_statistics_match_geogebra() {
    let ids = [
        "statistics-pairedstats-001",
        "statistics-pairedstats-002",
        "statistics-pairedalias-001",
        "statistics-pairedpoints-001",
        "statistics-pairedsamples-001",
        "statistics-pairedmissing-001",
        "statistics-pairedgeometry-001",
        "statistics-pairedselection-001",
        "statistics-pairedkindfreeze-001",
        "statistics-pairedwideindex-001",
        "statistics-pairedelementtypes-001",
        "statistics-pairedmixedrow-001",
        "statistics-pairedroworigin-001",
        "statistics-pairedrowcopies-001",
        "statistics-pairedrowdepth-001",
        "statistics-pairedcommanddeps-001",
        "statistics-pairedrowrounding-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn x_moments_match_geogebra() {
    let ids = [
        "statistics-centeredmoments-001",
        "statistics-centeredmoments-002",
        "statistics-momentsedges-001",
        "statistics-momentsangles-001",
        "statistics-momentsdispatch-001",
        "statistics-momentsgeometry-001",
        "statistics-momentsfallback-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn percentile_matches_geogebra() {
    let ids = [
        "statistics-percentileraw-001",
        "statistics-percentileweighted-001",
        "statistics-percentileundefined-001",
        "statistics-percentilesamples-001",
        "statistics-percentileround-001",
        "statistics-percentilegeometry-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn statistics_followup_matches_geogebra() {
    let ids = [
        "statistics-nextlists-001",
        "statistics-nextedges-001",
        "statistics-nextvariadic-001",
        "statistics-nextkinds-001",
        "statistics-nextedit-001",
        "statistics-nextpoints-001",
        "statistics-nextreject-001",
        "statistics-nextreject-002",
        "statistics-nextsegmentprecision-001",
        "statistics-nextundefinedboolean-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn spreadsheet_closing_regressions_match_geogebra() {
    let ids = [
        "spreadsheet-barerange-001",
        "spreadsheet-circularedit-001",
        "spreadsheet-clearrangemember-001",
        "spreadsheet-filloverlapbudget-001",
        "spreadsheet-liverangefill-001",
        "spreadsheet-radianmodelist-001",
        "spreadsheet-rangefillformula-001",
        "spreadsheet-rangefillnamedlist-001",
        "spreadsheet-scientificinput-001",
        "spreadsheet-segmentreload-001",
        "spreadsheet-sliderassignment-001",
        "spreadsheet-visibilityshow-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn homogeneous_point_cases_agree() {
    let ids = [
        "vector-ray-008",
        "vector-rawscale-008",
        "vector-rawscale-009",
        "predicates-infinity-001",
        "predicates-homogeneous-001",
        "points-infinity-001",
    ];
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| ids.contains(&case.id.as_str()));
    assert_eq!(cases.len(), ids.len());
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn projective_update_cases_agree() {
    let mut cases = model::load_all(&repo_root()).unwrap();
    cases.retain(|case| {
        case.id.contains("-projectiveupdates-")
            || matches!(
                case.id.as_str(),
                "points-infinityedit-007"
                    | "points-isolatedprojection-001"
                    | "locus-samplingbudget-001"
            )
    });
    assert_eq!(cases.len(), 16);
    let report = report::evaluate(&cases, &gaps::Gaps::new());
    assert!(report.failures.is_empty(), "{}", report.failures.join("\n"));
}

#[test]
fn kernel_agrees_with_golden_data() {
    if std::env::var_os("GEO_GOLDEN_REQUIRE_GIAC").is_some() {
        require_giac();
    }
    let mut cases = model::load_all(&repo_root()).unwrap_or_else(|e| panic!("golden data:\n{e}"));
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    apply_desktop_expectations(&mut cases);
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

#[cfg(not(feature = "giac"))]
fn require_giac() {
    panic!("the Giac golden profile requires the geo-golden giac feature");
}

#[cfg(feature = "giac")]
fn require_giac() {
    assert!(
        geo_kernel::giac_backend_active(),
        "the Giac golden profile requires geo-kernel/giac and a working Giac library"
    );

    // PlotSolve has no kernel-only implementation.
    let c = script::load("f(x) = x - 2\nresult = PlotSolve(f)").expect("run the Giac kernel probe");
    let points = c
        .elements(c.id("result").unwrap())
        .expect("Giac must return the probe's list of points");
    let points: Vec<_> = points.iter().collect();
    assert!(
        matches!(
            points.as_slice(),
            [geo_kernel::expr::Element::Value(
                Kind::Point,
                Value::Point { x: 2.0, y: 0.0 }
            )]
        ),
        "the Giac kernel probe must return [(2, 0)], got {points:?}"
    );
}

/// Apply only the recorded variant at each listed snapshot position. The
/// normal comparator still checks every element, coordinate, and object.
fn apply_desktop_expectations(cases: &mut [model::Case]) {
    use model::ExpectedData::{List, Number, Point};

    let intel_mac = cfg!(all(target_os = "macos", target_arch = "x86_64"));
    let windows = cfg!(all(target_os = "windows", target_arch = "x86_64"));
    if intel_mac {
        let point = desktop_snapshot(cases, "misc-maximizepoint-015", 0, "M");
        assert!(matches!(point, Point { .. }));
        // This is the optimizer's Intel result with either backend.
        *point = Point {
            x: 4.000000050592005,
            y: 3.9999999494079943,
        };
    }
    if !(intel_mac || windows) || !geo_kernel::giac_backend_active() {
        return;
    }

    type Order = (String, usize, String, Vec<usize>);
    let orders: Vec<Order> = serde_json::from_str(include_str!("../desktop-giac-orders.json"))
        .expect("recorded desktop Giac result orders");
    for (id, step, label, order) in orders {
        let List(elements) = desktop_snapshot(cases, &id, step, &label) else {
            panic!("{id} {step}:{label} no longer records a list");
        };
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert!(
            sorted.into_iter().eq(0..elements.len()),
            "{id} {step}:{label} has an invalid result permutation"
        );
        *elements = order
            .into_iter()
            .map(|index| elements[index].clone())
            .collect();
    }

    if windows {
        let List(elements) = desktop_snapshot(cases, "cas-edge-004", 0, "r2") else {
            panic!("cas-edge-004 0:r2 no longer records a list");
        };
        assert_eq!(elements.len(), 1);
        let Some(Number(value)) = &mut elements[0].data else {
            panic!("cas-edge-004 0:r2 no longer records a numeric solution");
        };
        *value = -8.881784197001E-6;
    }
}

fn desktop_snapshot<'a>(
    cases: &'a mut [model::Case],
    id: &str,
    step: usize,
    label: &str,
) -> &'a mut model::ExpectedData {
    cases
        .iter_mut()
        .find(|case| case.id == id)
        .and_then(|case| case.snapshots.get_mut(step))
        .and_then(|snapshot| snapshot.get_mut(label))
        .and_then(|object| object.data.as_mut())
        .unwrap_or_else(|| {
            panic!("desktop allow-list refers to missing object {id} {step}:{label}")
        })
}

#[test]
fn reduced_row_echelon_form_preserves_the_reference_zero_sign() {
    let cases = model::load_all(&repo_root()).unwrap();
    let case = cases
        .iter()
        .find(|c| c.id == "matrixdilate-elimination-011")
        .unwrap();
    let Some(model::ExpectedData::List(rows)) = &case.snapshots[0]["a"].data else {
        panic!("expected a matrix snapshot");
    };
    let Some(model::ExpectedData::List(row)) = &rows[0].data else {
        panic!("expected a matrix row");
    };
    let Some(model::ExpectedData::Number(want)) = row[1].data else {
        panic!("expected a number");
    };
    let c = script::load(&case.script.join("\n")).unwrap();
    let matrix = c.elements(c.id("a").unwrap()).unwrap();
    let Some(geo_kernel::expr::Element::List(row)) = matrix.get(0) else {
        panic!("expected a matrix row");
    };
    let Some(geo_kernel::expr::Element::Value(Kind::Number, Value::Number(actual))) = row.get(1)
    else {
        panic!("expected a number");
    };
    assert_eq!(actual.to_bits(), want.to_bits());
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

#[test]
fn a_rolled_back_addition_restores_take_slots() {
    let cases = model::load_all(&repo_root()).unwrap();
    let case = cases
        .iter()
        .find(|c| c.id == "keptvalues-checkpoint-002")
        .unwrap();
    let mut c = script::load(&case.script[..3].join("\n")).unwrap();
    let n = c.id("n").unwrap();
    assert_eq!(c.value(n), Value::Number(0.0));
    let checkpoint = c.checkpoint();
    script::exec(&mut c, &case.script[3]).unwrap();
    assert_eq!(c.value(n), Value::Number(2.0));
    c.rollback(checkpoint).unwrap();
    assert_eq!(c.value(n), Value::Number(0.0));
}
