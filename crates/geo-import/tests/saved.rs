//! Files saved after their free points were moved. `golden/ggb/saved/<id>.xml`
//! is the file GeoGebra writes for the state the harness reached after the
//! moves, `<id>.snapshot` what GeoGebra computed in that same state (see
//! `tools/ggb-harness/harness.sh saved`). Every object the importer gives
//! must agree with the snapshot, and none may disagree with the file's own
//! saved values: this proves that the n-th output slot of a saved command is
//! the n-th object the kernel makes, in the states that matter (a line or a
//! circle dragged until the two intersection points swap, tangents, angles
//! under both angle units).

use geo_golden::compare::compare;
use geo_golden::model::Expected;
use geo_golden::repo_root;
use geo_import::import;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

#[derive(Deserialize)]
struct Snapshot {
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
    assert!(names.len() >= 80, "only {} saved cases", names.len());
    let (mut compared, mut failures) = (0, Vec::new());
    for path in &names {
        let snap: Snapshot = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let xml = fs::read(path.with_extension("xml")).unwrap();
        let imported = import(&xml).unwrap();
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
    println!("saved files: {}, objects compared: {compared}", names.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(compared >= 300, "only {compared} objects compared");
}
