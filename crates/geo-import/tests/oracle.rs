//! The oracle test: every construction file that GeoGebra wrote for a golden
//! case (`golden/ggb/<group>/<case id>.xml`, plus a few zipped `.ggb`) is
//! imported, the script it gives runs on the kernel, and every imported object
//! is compared with `snapshots[0]` of the same golden case under the rules of
//! `golden/SCHEMA.md`, with the comparison of `geo-golden`.

use geo_golden::compare::compare;
use geo_golden::model::{self, Case};
use geo_golden::{gaps, repo_root};
use geo_import::import;
use geo_kernel::script::exec;
use geo_kernel::Construction;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// At the time of writing (see the assertions below for the exact counts).
const MIN_FILES: usize = 1151;
const MIN_ZIPS: usize = 27;
const MIN_COMPARED: usize = 7900;
/// Objects the importer may skip in the whole corpus: the transformed ray of
/// `vector-chain-002`, whose file writes `Vector[Translate[Direction[g], u]]`
/// as an argument (an expression), and what depends on it.
const MAX_SKIPPED: usize = 6;
/// Outputs without a label: GeoGebra labels an object only once it is defined.
const MAX_UNLABELLED: usize = 5;

fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

/// A reason without the names and numbers that differ from file to file.
fn reason_key(reason: &str) -> String {
    if reason.starts_with("depends on") {
        return "depends on a skipped object".into();
    }
    let mut key = String::new();
    let mut quoted = false;
    for ch in reason.chars() {
        match ch {
            '`' => {
                quoted = !quoted;
                key.push('`');
            }
            _ if quoted => {}
            _ => key.push(ch),
        }
    }
    key
}

#[derive(Default)]
struct Totals {
    files: usize,
    zips: usize,
    objects_compared: usize,
    objects_skipped: usize,
    gap_objects: usize,
    skipped_by_reason: BTreeMap<String, usize>,
    /// The first case that was skipped for each reason.
    example: BTreeMap<String, String>,
    /// Outputs without a label in the file: GeoGebra labels an object only
    /// once it is defined, so the snapshot must say undefined.
    unlabelled: usize,
    unlabelled_objects: usize,
    skipped_by_group: BTreeMap<String, usize>,
    hidden: usize,
}

fn compare_case(
    case: &Case,
    bytes: &[u8],
    gaps: &gaps::Gaps,
    import_gaps: &gaps::Gaps,
    used_import_gaps: &mut BTreeSet<String>,
    totals: &mut Totals,
    failures: &mut Vec<String>,
) -> Vec<String> {
    let imported = match import(bytes) {
        Ok(i) => i,
        Err(e) => {
            failures.push(format!("{}: import failed: {e}", case.id));
            return vec![];
        }
    };
    let mut c = Construction::new();
    for line in &imported.script {
        if let Err(e) = exec(&mut c, line) {
            failures.push(format!(
                "{}: the imported line `{line}` fails: {e}",
                case.id
            ));
            return imported.script;
        }
    }
    totals.hidden += imported.hidden.len();
    let skipped: HashMap<&str, &str> = imported
        .skipped
        .iter()
        .map(|s| (s.label.as_str(), s.reason.as_str()))
        .collect();
    let mut unlabelled = 0;
    for s in &imported.skipped {
        if s.label.is_empty() {
            unlabelled += 1;
            totals.unlabelled += 1;
            continue;
        }
        let key = format!("{}: {}", s.kind, reason_key(&s.reason));
        totals.example.entry(key.clone()).or_insert_with(|| {
            format!(
                "{} `{}`: {}\n    script: {:?}",
                case.id, s.label, s.reason, case.script
            )
        });
        *totals.skipped_by_reason.entry(key).or_default() += 1;
    }
    let mut missing = 0;
    let gap = gaps.get(&case.id);
    let import_gap = import_gaps.get(&case.id);
    for (label, expected) in &case.snapshots[0] {
        if skipped.contains_key(label.as_str()) {
            totals.objects_skipped += 1;
            *totals
                .skipped_by_group
                .entry(case.group.clone())
                .or_default() += 1;
            continue;
        }
        let Some(id) = c.id(label) else {
            // The file has no label for it, so it must be undefined.
            if expected.data.is_none() && missing < unlabelled {
                missing += 1;
                totals.unlabelled_objects += 1;
            } else {
                failures.push(format!(
                    "{}: `{label}` was neither imported nor skipped",
                    case.id
                ));
            }
            continue;
        };
        match compare(expected, c.kind(id), &c.value(id)) {
            Ok(_) => totals.objects_compared += 1,
            Err(why) => {
                let key = format!("0:{label}");
                if import_gap.is_some_and(|g| g.mismatches.contains(&key)) {
                    used_import_gaps.insert(format!("{}:{key}", case.id));
                } else if gap.is_some_and(|g| g.mismatches.contains(&key)) {
                    totals.gap_objects += 1;
                } else {
                    failures.push(format!(
                        "{}: `{label}` disagrees: {why}; expected {expected:?}, got {:?}",
                        case.id,
                        c.value(id)
                    ));
                }
            }
        }
    }
    imported.script
}

#[test]
fn imported_constructions_agree_with_the_golden_snapshots() {
    let root = repo_root();
    let cases: HashMap<String, Case> = model::load_all(&root)
        .unwrap()
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let gaps = gaps::load().unwrap();
    let import_gaps = gaps::parse(include_str!("../known-import-gaps.toml")).unwrap();
    let mut used_import_gaps = BTreeSet::new();
    let mut xml = Vec::new();
    files(&root.join("golden/ggb"), "xml", &mut xml);
    xml.sort();
    let mut totals = Totals::default();
    let mut failures = Vec::new();
    for path in &xml {
        let id = path.file_stem().unwrap().to_str().unwrap();
        let case = cases
            .get(id)
            .unwrap_or_else(|| panic!("no golden case for {}", path.display()));
        let script = compare_case(
            case,
            &fs::read(path).unwrap(),
            &gaps,
            &import_gaps,
            &mut used_import_gaps,
            &mut totals,
            &mut failures,
        );
        totals.files += 1;
        // The zipped file of the same case must give the same script.
        let zip = path.with_extension("ggb");
        if zip.exists() {
            totals.zips += 1;
            let zipped = import(&fs::read(&zip).unwrap()).unwrap();
            assert_eq!(zipped.script, script, "{}: zip and xml differ", case.id);
        }
    }
    println!(
        "files imported: {} (of which with a zipped twin: {})\nobjects compared: {}\n\
         objects in known gaps (disagree as listed): {}\n\
         objects in known import gaps (disagree as listed): {}\nobjects skipped: {}\n\
         hidden objects reported: {}\nskipped by group: {:?}",
        totals.files,
        totals.zips,
        totals.objects_compared,
        totals.gap_objects,
        used_import_gaps.len(),
        totals.objects_skipped,
        totals.hidden,
        totals.skipped_by_group
    );
    println!(
        "outputs without a label in the file: {} (snapshot objects undefined as required: {})",
        totals.unlabelled, totals.unlabelled_objects
    );
    for (reason, n) in &totals.skipped_by_reason {
        println!(
            "  skipped {n:5}  {reason}\n    e.g. {}",
            totals.example[reason]
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        totals.objects_skipped <= MAX_SKIPPED,
        "{} objects skipped, at most {MAX_SKIPPED} are documented",
        totals.objects_skipped
    );
    for key in totals.skipped_by_reason.keys() {
        assert!(
            key.contains("is an expression") || key.contains("depends on a skipped object"),
            "an undocumented reason for skipping: {key}"
        );
    }
    assert_eq!(
        totals.skipped_by_group.keys().collect::<Vec<_>>(),
        ["vector"]
    );
    assert!(
        totals.unlabelled <= MAX_UNLABELLED,
        "{} unlabelled",
        totals.unlabelled
    );
    assert_eq!(totals.unlabelled, totals.unlabelled_objects);
    let listed: BTreeSet<String> = import_gaps
        .iter()
        .flat_map(|(id, g)| g.mismatches.iter().map(move |m| format!("{id}:{m}")))
        .collect();
    assert_eq!(
        used_import_gaps, listed,
        "known-import-gaps.toml is stale or incomplete"
    );
    assert!(totals.files >= MIN_FILES, "only {} files", totals.files);
    assert!(totals.zips >= MIN_ZIPS, "only {} zip files", totals.zips);
    assert!(
        totals.objects_compared >= MIN_COMPARED,
        "only {} objects compared",
        totals.objects_compared
    );
}
