//! Files saved after edits. `golden/ggb/saved/<id>.xml` or `<id>.ggb`
//! is the file GeoGebra writes for the state the harness reached after the
//! moves, `<id>.snapshot` what GeoGebra computed in that same state (see
//! `tools/ggb-harness/harness.sh saved`). Every object the importer gives
//! must agree with the snapshot, and none may disagree with the file's own
//! saved values: this shows, for the states listed here only, that the n-th output
//! slot of a saved command is the n-th object the kernel makes (a line or a
//! circle dragged until the two intersection points swap, tangents, angles
//! under both angle units).

use geo_golden::compare::{
    compare, compare_conic, compare_construction_object, compare_list, compare_locus,
    compare_object, compare_sampled,
};
use geo_golden::model::{set_snapshot_schema, Expected, ExpectedData};
use geo_golden::repo_root;
use geo_import::import;
use geo_kernel::expr::{work_done, MAX_EVALUATION_WORK};
use geo_kernel::kernel::text::TextPosition;
use geo_kernel::{script, Construction, Kind, Value};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

#[derive(Deserialize)]
struct Snapshot {
    schema: u32,
    id: String,
    objects: BTreeMap<String, Expected>,
}

#[derive(Deserialize)]
struct TangencyGap {
    id: String,
    label: String,
    reason: String,
}

#[derive(Deserialize)]
struct TangencyGaps {
    saved_tangency: Vec<TangencyGap>,
}

const EXPRESSION_FIXTURES: [&str; 3] = [
    "saved-expressions-001",
    "saved-expressions-002",
    "saved-expressions-003",
];
const REVIEW_FIXTURES: [&str; 4] = [
    "saved-paths-001",
    "saved-paths-002",
    "saved-review-001",
    "saved-coordinates-001",
];
const TEXT_FIXTURES: [&str; 2] = ["saved-texts-001", "saved-texts-002"];
const TEXT_EQUATION_FIXTURES: [&str; 2] = ["saved-text-equations-001", "saved-text-equations-002"];
const TEXT_CONIC_STYLE_FIXTURE: &str = "saved-text-conic-styles-001";
const TEXT_TEMPLATE_FIXTURES: [&str; 2] = ["saved-text-template-001", "saved-text-template-002"];

const INLINE_FIXTURES: [&str; 2] = ["saved-inline-001", "saved-inline-002"];
const STYLE_FIXTURES: &str = "saved-styles-";

fn fixture(name: &str) -> (Snapshot, geo_import::Imported) {
    let dir = repo_root().join("golden/ggb/saved");
    let xml = fs::read(dir.join(format!("{name}.xml"))).unwrap();
    let mut snap: Snapshot =
        serde_json::from_slice(&fs::read(dir.join(format!("{name}.snapshot"))).unwrap()).unwrap();
    set_snapshot_schema(snap.schema, &mut snap.objects).unwrap();
    (snap, within_budget(|| import(&xml)).unwrap())
}

fn check_review_values(snap: &Snapshot, c: &Construction, skipped: &[&str]) {
    let labelled = c.ids().filter(|&id| !c.label(id).starts_with('\0')).count();
    assert_eq!(labelled, snap.objects.len() - skipped.len(), "{}", snap.id);
    for (label, expected) in &snap.objects {
        let id = c.id(label);
        if skipped.contains(&label.as_str()) {
            assert!(id.is_none(), "{}: {label}", snap.id);
            continue;
        }
        let id = id.unwrap_or_else(|| panic!("{}: missing {label}", snap.id));
        let kind = c.kind(id);
        let value = c.value(id);
        let result = match kind {
            Kind::List => compare_list(c, expected, kind, c.elements(id)),
            Kind::Conic => compare_conic(expected, kind, &value, c.conic(id).as_ref()).0,
            Kind::Locus => compare_locus(expected, kind, &value, c.locus(id)),
            Kind::Text => {
                assert_eq!(kind, expected.kind);
                match &expected.data {
                    Some(ExpectedData::Text(text)) => {
                        assert_eq!(c.text(id), Some(text.as_str()), "{}: {label}", snap.id)
                    }
                    None => assert_eq!(value, Value::Undefined),
                    other => panic!("{}: {label}: unexpected text snapshot {other:?}", snap.id),
                }
                continue;
            }
            _ => {
                let vertices: Vec<_> = c.vertices(id).collect();
                let sample = |x| within_budget(|| c.function_value(id, x));
                compare_sampled(expected, kind, &value, &vertices, Some(&sample))
            }
        };
        result.unwrap_or_else(|why| panic!("{}: {label}: {why}", snap.id));
    }
}

#[test]
fn saved_regular_polygon_keeps_undefined_generated_vertices() {
    use geo_import::export::{ggb, Definition, Presentation};

    let (snap, imported) = fixture("saved-keptvertices-001");
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    let replayed = script::load(&imported.script.join("\n")).unwrap();
    check_review_values(&snap, &replayed, &[]);

    let definitions = [
        ("n = 0", "n"),
        ("U = (0, 0)", "U"),
        ("A = If(n > 0, U)", "A"),
        ("B = (2, 0)", "B"),
        ("nv = 5", "nv"),
        ("p = $regularPolygon(A, B, nv)", "p C D E f g h i j"),
    ]
    .map(|(line, labels)| {
        (
            line,
            labels
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    });
    let file = ggb(
        &replayed,
        definitions
            .iter()
            .map(|(line, outputs)| Definition { line, outputs }),
        &Presentation::default(),
    )
    .unwrap();
    let reimported = import(&file).unwrap();
    assert!(reimported.skipped.is_empty(), "{:?}", reimported.skipped);
    check_review_values(
        &snap,
        &script::load(&reimported.script.join("\n")).unwrap(),
        &[],
    );
}

#[test]
fn saved_curve_intersections_keep_their_output_slots() {
    for name in [
        "saved-curve-intersections-001",
        "saved-curve-intersections-002",
    ] {
        let (snap, imported) = fixture(name);
        assert!(
            imported.skipped.is_empty(),
            "{name}: {:?}",
            imported.skipped
        );
        assert!(
            imported.mismatched.is_empty(),
            "{name}: {:?}",
            imported.mismatched
        );
        check_review_values(
            &snap,
            &script::load(&imported.script.join("\n")).unwrap(),
            &[],
        );
    }
}

#[test]
fn inline_command_inputs_are_live_and_create_only_saved_labels() {
    let (before, initial) = fixture(INLINE_FIXTURES[0]);
    let (after, edited) = fixture(INLINE_FIXTURES[1]);
    for (snap, imported) in [(&before, &initial), (&after, &edited)] {
        assert!(
            imported.skipped.is_empty(),
            "{}: {:?}",
            snap.id,
            imported.skipped
        );
        assert!(
            imported.mismatched.is_empty(),
            "{}: {:?}",
            snap.id,
            imported.mismatched
        );
        check_review_values(
            snap,
            &script::load(&imported.script.join("\n")).unwrap(),
            &[],
        );
    }
    let mut live = script::load(&initial.script.join("\n")).unwrap();
    live.move_point(live.id("A").unwrap(), 4.0, 1.0).unwrap();
    live.set_number(live.id("a").unwrap(), 2.0).unwrap();
    check_review_values(&after, &live, &[]);
}

#[test]
fn saved_style_fixtures_import_every_object_with_a_style() {
    let dir = repo_root().join("golden/ggb/saved");
    let mut fixtures = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with(STYLE_FIXTURES) || name.ends_with(".snapshot") {
            continue;
        }
        fixtures += 1;
        let imported = within_budget(|| import(&fs::read(&path).unwrap())).unwrap();
        assert!(
            imported.skipped.is_empty(),
            "{name}: {:?}",
            imported.skipped
        );
        assert!(
            imported.mismatched.is_empty(),
            "{name}: {:?}",
            imported.mismatched
        );
        // No saved expression of a dynamic colour or an animation was dropped.
        assert!(
            !imported
                .notes
                .iter()
                .any(|note| note.contains("dynamic colour") || note.contains("animation")),
            "{name}: {:?}",
            imported.notes
        );
        let snapshot: serde_json::Value =
            serde_json::from_slice(&fs::read(path.with_extension("snapshot")).unwrap()).unwrap();
        for label in snapshot["objects"].as_object().unwrap().keys() {
            assert!(
                imported.object_styles.contains_key(label),
                "{name}: no style for {label}"
            );
        }
        // The helpers of every fixture are there, and something is styled.
        for helper in ["r", "g", "b", "show", "note"] {
            assert!(imported.object_styles.contains_key(helper), "{name}");
        }
        assert!(
            imported
                .object_styles
                .values()
                .any(|style| style.color.is_some() && style.layer.is_some()),
            "{name}"
        );
    }
    assert_eq!(fixtures, 13);
}

/// A file of pen strokes holds every number in five significant digits
/// (`GeoLocusStroke.appendBezierCoords`): the importer restores exactly
/// those numbers, which are the values GeoGebra computed when saving,
/// rounded so, with the same segment codes and breaks.
#[test]
fn saved_strokes_restore_the_rounded_points_of_the_file() {
    let (snap, imported) = fixture("saved-strokes-001");
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let c = script::load(&imported.script.join("\n")).unwrap();
    let rounded = |v: f64| -> f64 {
        geo_kernel::algo::stroke::reference_number(v)
            .parse()
            .unwrap()
    };
    let mut strokes = 0;
    for (label, expected) in &snap.objects {
        let id = c.id(label).unwrap();
        assert_eq!(c.kind(id), Kind::PenStroke, "{label}");
        let stroke = c.stroke(id).unwrap();
        match &expected.data {
            Some(ExpectedData::Stroke(points)) if !points.is_empty() => {
                assert!(stroke.defined, "{label}");
                assert_eq!(points.len(), stroke.points.len(), "{label}");
                for (i, (&(x, y, code), p)) in points.iter().zip(stroke.points.iter()).enumerate() {
                    match x {
                        Some(x) => {
                            assert_eq!(p.x, rounded(x), "{label} point {i}");
                            assert_eq!(p.y, rounded(y.unwrap()), "{label} point {i}");
                            assert_eq!(p.segment.code(), code, "{label} point {i}");
                        }
                        None => assert!(!p.is_defined(), "{label} point {i}"),
                    }
                }
                strokes += 1;
            }
            // The file has an empty value for a stroke without points, and
            // the reader leaves an empty value unapplied: undefined, as
            // `PenStroke()` itself.
            _ => assert!(!c.value(id).is_defined(), "{label}"),
        }
    }
    assert_eq!(strokes, 4);
}

#[test]
fn saved_elements_rebuild_their_geometry() {
    let (snap, imported) = fixture("saved-elements-001");
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let c = script::load(&imported.script.join("\n")).unwrap();
    check_review_values(&snap, &c, &[]);
}

#[test]
fn saved_text_objects_match_before_and_after_edits() {
    let dir = repo_root().join("golden/ggb/saved");
    let load = |name: &str| {
        let bytes = fs::read(dir.join(format!("{name}.ggb"))).unwrap();
        let imported = import(&bytes).unwrap();
        let mut snap: Snapshot =
            serde_json::from_slice(&fs::read(dir.join(format!("{name}.snapshot"))).unwrap())
                .unwrap();
        set_snapshot_schema(snap.schema, &mut snap.objects).unwrap();
        assert!(
            imported.skipped.is_empty(),
            "{name}: {:?}",
            imported.skipped
        );
        assert!(
            imported.mismatched.is_empty(),
            "{name}: {:?}",
            imported.mismatched
        );
        assert!(imported.hidden.iter().any(|name| name == "hiddenText"));
        let c = script::load(&imported.script.join("\n")).unwrap();
        check_review_values(&snap, &c, &[]);
        let style = |label| c.text_presentation(c.id(label).unwrap()).unwrap();
        assert_eq!(
            style("caption").position,
            TextPosition::Screen { x: 51.0, y: 73.0 }
        );
        assert_eq!(
            style("note").position,
            TextPosition::World { x: -3.0, y: 2.0 }
        );
        assert_eq!(
            style("marker").position,
            TextPosition::Point(c.id("P").unwrap())
        );
        assert_eq!(
            style("atPoint").position,
            TextPosition::Point(c.id("P").unwrap())
        );
        assert_eq!(style("atPoint").horizontal, Some(0));
        assert_eq!(style("atPoint").vertical, Some(1));
        let formula = style("formula");
        assert!(formula.latex && formula.serif && formula.bold && formula.italic);
        assert_eq!(formula.size, 1.4);
        (snap, c)
    };
    let (_, mut live) = load(TEXT_FIXTURES[0]);
    let (after, _) = load(TEXT_FIXTURES[1]);
    let ExpectedData::Point { x, y } = after.objects["P"].data.as_ref().unwrap() else {
        panic!("expected a point");
    };
    let ExpectedData::Number(n) = after.objects["a"].data.as_ref().unwrap() else {
        panic!("expected a number");
    };
    live.move_point(live.id("P").unwrap(), *x, *y).unwrap();
    live.set_number(live.id("a").unwrap(), *n).unwrap();
    check_review_values(&after, &live, &[]);
}

#[test]
fn saved_styles_are_used_to_recompute_texts() {
    for pair in [TEXT_EQUATION_FIXTURES, TEXT_TEMPLATE_FIXTURES] {
        check_saved_text_recompute(pair);
    }
}

#[test]
fn text_matches_saved_conic_equation_styles() {
    let (snapshot, imported) = fixture(TEXT_CONIC_STYLE_FIXTURE);
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let c = script::load(&imported.script.join("\n")).unwrap();
    check_review_values(&snapshot, &c, &[]);
}

fn check_saved_text_recompute(pair: [&str; 2]) {
    let [before, after] = pair.map(fixture);
    let load = |(snapshot, imported): &(Snapshot, geo_import::Imported)| {
        assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
        assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
        let c = script::load(&imported.script.join("\n")).unwrap();
        check_review_values(snapshot, &c, &[]);
        c
    };
    let mut live = load(&before);
    load(&after);
    let Some(ExpectedData::Number(a)) = after.0.objects["a"].data else {
        panic!("expected a number");
    };
    live.set_number(live.id("a").unwrap(), a).unwrap();
    check_review_values(&after.0, &live, &[]);
}

#[test]
fn saved_dragged_path_points_restore_their_positions_and_follow_edits() {
    let (start, first) = fixture(REVIEW_FIXTURES[0]);
    let (after, second) = fixture(REVIEW_FIXTURES[1]);
    for (snap, imported) in [(&start, &first), (&after, &second)] {
        assert!(
            imported.skipped.is_empty(),
            "{}: {:?}",
            snap.id,
            imported.skipped
        );
        assert!(
            imported.mismatched.is_empty(),
            "{}: {:?}",
            snap.id,
            imported.mismatched
        );
        let c = within_budget(|| script::load(&imported.script.join("\n"))).unwrap();
        check_review_values(snap, &c, &[]);
    }
    let mut live = script::load(&first.script.join("\n")).unwrap();
    for (label, x, y) in [("B", 7.0, 3.0), ("A", -1.0, 1.0), ("F", -2.5, 2.0)] {
        live.move_point(live.id(label).unwrap(), x, y).unwrap();
    }
    check_review_values(&after, &live, &[]);
}

#[test]
fn saved_angles_and_if_keep_their_semantics() {
    let (snap, imported) = fixture(REVIEW_FIXTURES[2]);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let c = script::load(&imported.script.join("\n")).unwrap();
    check_review_values(&snap, &c, &["e", "lost"]);
}

#[test]
fn skipped_labels_shadow_constants() {
    let (_, imported) = fixture(REVIEW_FIXTURES[2]);
    assert!(
        imported
            .skipped
            .iter()
            .any(|s| s.label == "lost" && s.reason == "depends on the skipped object `e`"),
        "{:?}",
        imported.skipped
    );
}

#[test]
fn independent_expressions_keep_saved_values_and_definitions() {
    let (_, imported) = fixture(REVIEW_FIXTURES[2]);
    let mut c = script::load(&imported.script.join("\n")).unwrap();
    let a = c.id("a").unwrap();
    let p = c.id("A").unwrap();
    let Value::Number(n) = c.value(a) else {
        panic!("expected number")
    };
    let Value::Point { x, .. } = c.value(p) else {
        panic!("expected point")
    };
    assert_eq!(n.to_bits(), std::f64::consts::TAU.to_bits());
    assert_eq!(x.to_bits(), std::f64::consts::SQRT_2.to_bits());
    for part in ["sqrt(2)", "2 * pi", "1 / 3"] {
        assert!(
            imported.script.iter().any(|line| line.contains(part)),
            "{part}"
        );
    }
    c.set_number(a, 2.0).unwrap();
    c.move_point(p, 2.0, 1.0).unwrap();
    assert_eq!(
        c.value(c.id("chosen").unwrap()),
        Value::Point { x: 2.0, y: 1.0 }
    );
}

#[test]
fn forced_coordinates_match_harness_values_and_reject_three_coordinates() {
    let (snap, imported) = fixture(REVIEW_FIXTURES[3]);
    for (label, expected) in &snap.objects {
        assert!(imported.skipped.is_empty());
        assert!(imported.mismatched.is_empty());
        assert_eq!(
            expected.kind,
            if ["P", "Q"].contains(&label.as_str()) {
                Kind::Point
            } else if ["v", "w"].contains(&label.as_str()) {
                Kind::Vector
            } else {
                Kind::Number
            }
        );
    }
    let c = script::load("a = 2\nb = 3\nP = $point(a, b)\nQ = $point(1/2, 3)\nv = $vector(a, b)\nw = $vector(1/2, 3)").unwrap();
    check_review_values(&snap, &c, &[]);
    let c = script::load(&imported.script.join("\n")).unwrap();
    check_review_values(&snap, &c, &[]);
    assert_eq!(
        script::load("v = $vector(1,2,3)").err().unwrap().message,
        "`$vector` needs two coordinates"
    );
}

#[test]
fn saved_expressions_match_geogebra_before_and_after_edits() {
    let dir = repo_root().join("golden/ggb/saved");
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../tools/ggb-harness/saved/saved.json")).unwrap();
    let mut live: Option<Construction> = None;
    for (stage, file) in EXPRESSION_FIXTURES.into_iter().enumerate() {
        let expected_skips = BTreeMap::from([
            ("bad", "Limit"),
            ("lost", "depends on the skipped object `bad`"),
        ]);
        let snapshot: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join(format!("{file}.snapshot"))).unwrap())
                .unwrap();
        let imported =
            within_budget(|| import(&fs::read(dir.join(format!("{file}.xml"))).unwrap())).unwrap();
        assert!(
            imported.mismatched.is_empty(),
            "{file}: {:?}",
            imported.mismatched
        );
        assert_eq!(
            imported.skipped.len(),
            expected_skips.len(),
            "{file}: {:?}",
            imported.skipped
        );
        for skipped in &imported.skipped {
            assert!(
                skipped
                    .reason
                    .contains(expected_skips[skipped.label.as_str()]),
                "{file}: {skipped:?}"
            );
        }
        for hidden in ["C", "s", "v_{0x}", "valid"] {
            assert!(
                imported.hidden.iter().any(|label| label == hidden),
                "{file}: {hidden}"
            );
        }
        assert!(!imported.hidden.iter().any(|label| label == "Q"));

        let replay = within_budget(|| script::load(&imported.script.join("\n"))).unwrap();
        if let Some(live) = &mut live {
            let case = cases["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == file)
                .unwrap();
            let step = &case["steps"][stage - 1];
            let id = live.id(step["label"].as_str().unwrap()).unwrap();
            within_budget(|| match step["action"].as_str().unwrap() {
                "move" => {
                    live.move_point(id, step["x"].as_f64().unwrap(), step["y"].as_f64().unwrap())
                }
                "set_number" => live.set_number(id, step["value"].as_f64().unwrap()),
                action => panic!("unexpected step: {action}"),
            })
            .unwrap();
        }
        let mut expectations = BTreeMap::new();
        for (label, value) in snapshot["objects"].as_object().unwrap() {
            if ["button", "field"].contains(&label.as_str()) {
                assert!(replay.id(label).is_some(), "{file}: {label}");
                continue;
            }
            if expected_skips.contains_key(label.as_str()) {
                assert!(replay.id(label).is_none(), "{file}: {label}");
            } else {
                expectations.insert(
                    label.clone(),
                    serde_json::from_value(value.clone()).unwrap(),
                );
            }
        }
        set_snapshot_schema(
            snapshot["schema"].as_u64().unwrap() as u32,
            &mut expectations,
        )
        .unwrap();
        for c in std::iter::once(&replay).chain(live.as_ref()) {
            assert_eq!(c.len(), expectations.len() + 2, "{file}");
            for (label, expected) in &expectations {
                let id = c
                    .id(label)
                    .unwrap_or_else(|| panic!("{file}: missing {label}"));
                let multivariate = label == "multi";
                if multivariate {
                    assert_eq!(c.kind(id), Kind::FunctionNVar);
                }
                let sample = |x| {
                    within_budget(|| {
                        if multivariate {
                            c.function_value_xy(id, x, 0.0)
                        } else {
                            c.function_value(id, x)
                        }
                    })
                };
                let (kind, value) = if multivariate {
                    (Kind::Function, Value::Function)
                } else {
                    (c.kind(id), c.value(id))
                };
                compare_sampled(expected, kind, &value, &[], Some(&sample))
                    .unwrap_or_else(|why| panic!("{file}: {label}: {why}"));
            }
        }
        if live.is_none() {
            live = Some(replay);
        }
    }
}

fn within_budget<T>(run: impl FnOnce() -> T) -> T {
    let before = work_done();
    let result = run();
    assert!(work_done() - before <= MAX_EVALUATION_WORK);
    result
}

#[test]
fn files_saved_after_moves_agree_with_what_geogebra_computed_when_saving() {
    let dir = repo_root().join("golden/ggb/saved");
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "snapshot"))
        // The export oracle reloads the style fixtures and compares them.
        .filter(|p| {
            !p.file_stem()
                .unwrap()
                .to_string_lossy()
                .starts_with(STYLE_FIXTURES)
        })
        .filter(|p| {
            !EXPRESSION_FIXTURES
                .iter()
                .chain(&REVIEW_FIXTURES)
                .chain(&INLINE_FIXTURES)
                .chain(&[
                    "saved-controls-001",
                    "saved-controls-002",
                    "saved-elements-001",
                    // images.rs checks the archive assets and quad snapshots.
                    "saved-images-001",
                    "saved-images-002",
                    "saved-vertex-image-001",
                    "saved-vertex-image-002",
                    "saved-vertex-image-003",
                    // Its values are deliberately stale. The replay test checks the clamped values.
                    "saved-slider-bounds-001",
                    // The loaded self-intersection differs from its live
                    // snapshot; its file-load behaviour is tested separately.
                    "saved-keptvalues-002",
                    // Its numbers are rounded in the file; checked above.
                    "saved-strokes-001",
                    // `tests/export.rs` compares the visibility and the
                    // slider animations SlowPlot set with the export.
                    "saved-slowplot-001",
                ])
                .chain(&TEXT_FIXTURES)
                .chain(&TEXT_EQUATION_FIXTURES)
                .chain(&TEXT_TEMPLATE_FIXTURES)
                .chain(std::iter::once(&TEXT_CONIC_STYLE_FIXTURE))
                .any(|id| p.file_stem().unwrap() == *id)
        })
        .collect();
    names.sort();
    assert!(names.len() >= 116, "only {} saved cases", names.len());
    let (mut compared, mut failures) = (0, Vec::new());
    let mut first_moves = 0;
    let gaps: TangencyGaps = toml::from_str(include_str!("../known-import-gaps.toml")).unwrap();
    let gap_count = gaps.saved_tangency.len();
    let mut double_tangents: BTreeMap<_, _> = gaps
        .saved_tangency
        .into_iter()
        .map(|gap| (gap.id, (gap.label, gap.reason)))
        .collect();
    assert_eq!(double_tangents.len(), gap_count);
    for path in &names {
        let mut snap: Snapshot = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        set_snapshot_schema(snap.schema, &mut snap.objects).unwrap();
        let xml = fs::read(path.with_extension("xml")).unwrap();
        let imported = import(&xml).unwrap();
        if snap.id.starts_with("saved-firstmove-") {
            first_moves += 1;
        }
        let tangency = double_tangents.remove(&snap.id);
        if let Some((label, reason)) = &tangency {
            // `avoidDoubleTangentPoint` keeps a labelled second output at the
            // tangent point, equal to the first; the kernel makes the second
            // intersection undefined there (known gap, DIFFERENCES.md). It
            // must be reported as exactly that one mismatch.
            let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
            assert!(!reason.is_empty());
            let second = c.id(label).unwrap();
            assert!(
                !c.value(second).is_defined(),
                "{}: {:?} {:?}",
                snap.id,
                c.value(second),
                imported.mismatched
            );
            assert!(snap.objects[label].data.is_some(), "{}", snap.id);
            assert!(imported.skipped.is_empty(), "{}", snap.id);
            let labels: Vec<_> = imported
                .mismatched
                .iter()
                .map(|m| m.label.as_str())
                .collect();
            assert_eq!(labels, [label], "{}", snap.id);
            assert!(compare(
                &snap.objects["P_{1}"],
                geo_kernel::Kind::Point,
                &c.value(c.id("P_{1}").unwrap())
            )
            .is_ok());
        } else if !imported.skipped.is_empty() || !imported.mismatched.is_empty() {
            failures.push(format!(
                "{}: skipped {:?}, mismatched {:?}",
                snap.id, imported.skipped, imported.mismatched
            ));
        }
        let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
        for (label, expected) in &snap.objects {
            if tangency
                .as_ref()
                .is_some_and(|(gap_label, _)| gap_label == label)
            {
                continue;
            }
            let Some(id) = c.id(label) else {
                // An object that is undefined when saved has no label in the file.
                if expected.data.is_some() {
                    failures.push(format!("{}: `{label}` was not imported", snap.id));
                }
                continue;
            };
            let kind = c.kind(id);
            let vertices = if matches!(kind, Kind::Polygon | Kind::PolyLine) {
                c.vertices(id).collect()
            } else {
                Vec::new()
            };
            let agrees = match kind {
                Kind::ImplicitCurve => compare_construction_object(expected, &c, id),
                Kind::List => compare_list(&c, expected, kind, c.elements(id)),
                _ => compare_object(expected, kind, &c.value(id), &vertices),
            };
            match agrees {
                Ok(_) => compared += 1,
                Err(why) => failures.push(format!(
                    "{}: `{label}` disagrees: {why}; expected {expected:?}, got {:?}",
                    snap.id,
                    c.value(id)
                )),
            }
        }
    }
    assert_eq!(gap_count, 12);
    assert!(double_tangents.is_empty(), "{double_tangents:?}");
    assert_eq!(first_moves, 24);
    println!("saved files: {}, objects compared: {compared}", names.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(compared >= 300, "only {compared} objects compared");
}

/// Older stroke vertices of three or more points are smoothed in whatever
/// view the reference reader has, which the file does not fix: the stroke
/// is skipped with that reason instead of a guess. The saved stroke limit
/// of the kernel is the attribute limit of the importer.
#[test]
fn older_stroke_vertices_that_need_a_view_are_skipped() {
    let xml = br#"<geogebra format="5.0"><construction>
<expression label="t" exp="PenStroke[]"/>
<element type="penstroke" label="t"><strokeCoords val="-3,3,4,3,2,5"/></element>
<expression label="s" exp="PenStroke[]"/>
<element type="penstroke" label="s"><strokeCoords val="0,0,1,1"/></element>
</construction></geogebra>"#;
    let imported = import(xml).unwrap();
    assert_eq!(imported.skipped.len(), 1, "{:?}", imported.skipped);
    assert_eq!(imported.skipped[0].label, "t");
    assert!(imported.skipped[0].reason.contains("view"));
    let c = script::load(&imported.script.join("\n")).unwrap();
    let s = c.stroke(c.id("s").unwrap()).unwrap();
    assert_eq!(s.points.len(), 3);
    assert_eq!(
        geo_kernel::algo::stroke::MAX_SAVED_COORDS_BYTES,
        geo_import::xml::MAX_ATTRIBUTE_BYTES
    );
}

#[test]
fn an_undefined_regular_polygon_reloads_its_saved_vertices() {
    let file = br#"<geogebra><construction>
      <element type="point" label="A"><coords x="-6" y="1" z="1"/></element>
      <element type="point" label="B"><coords x="-2" y="2" z="1"/></element>
      <element type="point" label="O"><coords x="3" y="0" z="1"/></element>
      <element type="numeric" label="r"><value val="0.5"/></element>
      <command name="Circle"><input a0="O" a1="r"/><output a0="k"/></command>
      <command name="Line"><input a0="A" a1="B"/><output a0="l"/></command>
      <command name="Intersect"><input a0="l" a1="k" a2="1"/><output a0="X"/></command>
      <element type="point" label="C"><coords x="4" y="-4" z="1"/></element>
      <command name="Polygon"><input a0="X" a1="C" a2="4"/>
        <output a0="p" a1="D" a2="E"/></command>
      <element type="point" label="D">
        <coords x="9.543047208620575" y="3.827811165517703" z="1"/>
      </element>
      <element type="point" label="E">
        <coords x="1.7152360431028701" y="9.37085837413828" z="1"/>
      </element>
      <expression label="dx" exp="x(D)" type="numeric"/>
      <element type="numeric" label="dx"><value val="9.543047208620575"/></element>
    </construction></geogebra>"#;
    let imported = import(file).unwrap();
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let mut c = script::load(&imported.script.join("\n")).unwrap();
    let (p, d, dx, r) = (
        c.id("p").unwrap(),
        c.id("D").unwrap(),
        c.id("dx").unwrap(),
        c.id("r").unwrap(),
    );
    assert_eq!(c.value(p), Value::Undefined);
    assert_eq!(c.value(dx), Value::Number(9.543047208620575));
    c.set_number(r, 6.0).unwrap();
    assert!(c.value(p).is_defined());
    let updated = c.value(d);
    let Value::Point { x, .. } = updated else {
        panic!("the generated vertex was not updated");
    };
    assert_ne!(x, 9.543047208620575);
    c.set_number(r, 0.5).unwrap();
    assert_eq!(c.value(p), Value::Undefined);
    assert_eq!(c.value(d), updated);
}

#[test]
fn a_saved_self_intersection_follows_the_reference_load_order() {
    let (_, imported) = fixture("saved-keptvalues-002");
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    let c = script::load(&imported.script.join("\n")).unwrap();
    let value = |label| c.value(c.id(label).unwrap());
    assert_eq!(value("P"), Value::Undefined);
    assert!(value("X").is_defined());
    assert_eq!(value("Q"), value("X"));
}
