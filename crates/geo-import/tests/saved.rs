//! Files saved after edits. `golden/ggb/saved/<id>.xml` or `<id>.ggb`
//! is the file GeoGebra writes for the state the harness reached after the
//! moves, `<id>.snapshot` what GeoGebra computed in that same state (see
//! `tools/ggb-harness/harness.sh saved`). Every object the importer gives
//! must agree with the snapshot, and none may disagree with the file's own
//! saved values: this shows, for the states listed here only, that the n-th output
//! slot of a saved command is the n-th object the kernel makes (a line or a
//! circle dragged until the two intersection points swap, tangents, angles
//! under both angle units).

use geo_golden::compare::{compare, compare_conic, compare_list, compare_sampled};
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
            ("bad", "NextPrime"),
            ("lost", "depends on the skipped object `bad`"),
            ("multi", "function with multiple variables"),
            ("control", "checkbox control"),
            ("button", "button control"),
            ("field", "text field control"),
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
            assert_eq!(c.len(), expectations.len(), "{file}");
            for (label, expected) in &expectations {
                let id = c
                    .id(label)
                    .unwrap_or_else(|| panic!("{file}: missing {label}"));
                let sample = |x| within_budget(|| c.function_value(id, x));
                compare_sampled(expected, c.kind(id), &c.value(id), &[], Some(&sample))
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
        .filter(|p| {
            !EXPRESSION_FIXTURES
                .iter()
                .chain(&REVIEW_FIXTURES)
                .chain(&INLINE_FIXTURES)
                .chain(&[
                    "saved-controls-001",
                    "saved-controls-002",
                    "saved-elements-001",
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
    let mut first_moves = Vec::new();
    let mut double_tangents = 0;
    for path in &names {
        let mut snap: Snapshot = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        set_snapshot_schema(snap.schema, &mut snap.objects).unwrap();
        let xml = fs::read(path.with_extension("xml")).unwrap();
        let imported = import(&xml).unwrap();
        let first_move = snap.id.starts_with("saved-firstmove-");
        if first_move {
            // GeoGebra fixes the order of the two points at the first update
            // after creation; the kernel does not remember it. Where the two
            // orders differ, the file's values disagree with the kernel's
            // and must show up in `mismatched`, never silently.
            first_moves.push((snap.id.clone(), imported.mismatched.len()));
            let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
            if !snap.objects.contains_key("P_{2}") {
                println!("{}: labels {:?}", snap.id, snap.objects.keys());
                continue;
            }
            let value = |l: &str| c.value(c.id(l).unwrap());
            let swapped = |e: &Expected, v: geo_kernel::Value| {
                compare(e, geo_kernel::Kind::Point, &v).is_ok()
            };
            if imported.mismatched.is_empty() {
                for l in ["P_{1}", "P_{2}"] {
                    assert!(swapped(&snap.objects[l], value(l)), "{}: {l}", snap.id);
                }
            } else {
                // exactly the two points, each holding the other's place
                assert_eq!(imported.mismatched.len(), 2, "{}", snap.id);
                assert!(
                    swapped(&snap.objects["P_{1}"], value("P_{2}"))
                        && swapped(&snap.objects["P_{2}"], value("P_{1}")),
                    "{}: not a swap",
                    snap.id
                );
            }
            continue;
        }
        if snap.id.starts_with("saved-doubletangent-") {
            // `avoidDoubleTangentPoint` keeps a labelled second output at the
            // tangent point, equal to the first; the kernel makes the second
            // intersection undefined there (known gap, DIFFERENCES.md). It
            // must be reported as exactly that one mismatch.
            let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
            let second = c.id("P_{2}").unwrap();
            assert!(
                !c.value(second).is_defined(),
                "{}: {:?} {:?}",
                snap.id,
                c.value(second),
                imported.mismatched
            );
            assert!(snap.objects["P_{2}"].data.is_some(), "{}", snap.id);
            assert!(imported.skipped.is_empty(), "{}", snap.id);
            let labels: Vec<_> = imported
                .mismatched
                .iter()
                .map(|m| m.label.as_str())
                .collect();
            assert_eq!(labels, ["P_{2}"], "{}", snap.id);
            assert!(compare(
                &snap.objects["P_{1}"],
                geo_kernel::Kind::Point,
                &c.value(c.id("P_{1}").unwrap())
            )
            .is_ok());
            double_tangents += 1;
            continue;
        }
        if !imported.skipped.is_empty() || !imported.mismatched.is_empty() {
            failures.push(format!(
                "{}: skipped {:?}, mismatched {:?}",
                snap.id, imported.skipped, imported.mismatched
            ));
        }
        let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
        for (label, expected) in &snap.objects {
            let Some(id) = c.id(label) else {
                // An object that is undefined when saved has no label in the file.
                if expected.data.is_some() {
                    failures.push(format!("{}: `{label}` was not imported", snap.id));
                }
                continue;
            };
            match compare(expected, c.kind(id), &c.value(id)) {
                Ok(_) => compared += 1,
                Err(why) => failures.push(format!(
                    "{}: `{label}` disagrees: {why}; expected {expected:?}, got {:?}",
                    snap.id,
                    c.value(id)
                )),
            }
        }
    }
    // Recorded with GeoGebra: dragging a line through reversal as the very
    // first update swaps the two points of every line-circle case (12 of 12),
    // and of 5 of the 12 circle-circle cases; DIFFERENCES.md has the row.
    let swapped = first_moves.iter().filter(|(_, n)| *n > 0).count();
    assert_eq!(double_tangents, 12);
    assert_eq!(first_moves.len(), 24);
    assert_eq!(swapped, 17, "{first_moves:?}");
    assert!(first_moves[..12].iter().all(|(_, n)| *n == 2));
    println!("saved files: {}, objects compared: {compared}", names.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(compared >= 300, "only {compared} objects compared");
}
