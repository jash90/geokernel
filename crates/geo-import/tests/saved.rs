//! Files saved after their free points were moved. `golden/ggb/saved/<id>.xml`
//! is the file GeoGebra writes for the state the harness reached after the
//! moves, `<id>.snapshot` what GeoGebra computed in that same state (see
//! `tools/ggb-harness/harness.sh saved`). Every object the importer gives
//! must agree with the snapshot, and none may disagree with the file's own
//! saved values: this shows, for the states listed here only, that the n-th output
//! slot of a saved command is the n-th object the kernel makes (a line or a
//! circle dragged until the two intersection points swap, tangents, angles
//! under both angle units).

use geo_golden::compare::compare;
use geo_golden::model::{set_snapshot_schema, Expected};
use geo_golden::repo_root;
use geo_import::import;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

#[derive(Deserialize)]
struct Snapshot {
    schema: u32,
    id: String,
    objects: BTreeMap<String, Expected>,
}

#[test]
fn files_saved_after_moves_agree_with_what_geogebra_computed_when_saving() {
    let dir = repo_root().join("golden/ggb/saved");
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "snapshot"))
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
