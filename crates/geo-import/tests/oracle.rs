//! The oracle test: every construction file that GeoGebra wrote for a golden
//! case (`golden/ggb/<group>/<case id>.xml`, plus a few zipped `.ggb`) is
//! imported, the script it gives runs on the kernel, and every imported object
//! is compared with `snapshots[0]` of the same golden case under the rules of
//! `golden/SCHEMA.md`, with the comparison of `geo-golden`.

use geo_golden::compare::{compare_object, numbers_agree};
use geo_golden::model::ExpectedData;
use geo_golden::model::{self, Case};
use geo_golden::{gaps, repo_root};
use geo_import::import;
use geo_kernel::script::exec;
use geo_kernel::Construction;
use geo_kernel::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// At the time of writing (see the assertions below for the exact counts).
const MIN_FILES: usize = 1339;
const MIN_ZIPS: usize = 31;
const MIN_COMPARED: usize = 10390;
/// Objects the importer may skip in the whole corpus (listed in `EXPECTED_SKIPPED`).
const MAX_SKIPPED: usize = 10;
/// Every object the importer may skip, as `case:label`. Three are a `Ray`
/// whose file writes an expression as an argument (`ga` of `vector-chain-002`
/// and `t` of `vector-translate-014` and `-015`); `gb`, `d` and `q` of
/// `vector-chain-002` depend on them. `polygon-small-001` is `Polygon(A)`
/// of one point, which GeoGebra saves with the list `{A}` as its argument (an
/// expression), and the three objects measured on it. Nothing else may be
/// skipped.
const EXPECTED_SKIPPED: &[&str] = &[
    "vector-chain-002:d",
    "vector-chain-002:ga",
    "vector-chain-002:gb",
    "vector-chain-002:q",
    "polygon-small-001:p",
    "polygon-small-001:ar",
    "polygon-small-001:pe",
    "polygon-small-001:M",
    "vector-translate-014:t",
    "vector-translate-015:t",
];

/// An entry of `known-import-gaps.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportGap {
    id: String,
    label: String,
    golden: f64,
    imported: f64,
    reason: String,
}

/// An object whose imported value is the golden one, but whose saved conic
/// matrix GeoGebra reads as a different object when the file is loaded: the
/// live object was classified from the state of the conic it was made from,
/// the saved matrix is classified afresh.
#[derive(Deserialize)]
struct Reclassified {
    id: String,
    label: String,
    /// The saved value exactly as the cross-check reports it.
    saved: String,
    reason: String,
}

#[derive(Deserialize)]
struct ImportGapFile {
    gap: Vec<ImportGap>,
    #[serde(default)]
    reclassified: Vec<Reclassified>,
}

/// Whether two kernel values are the same within the tolerance of the golden files.
fn same_value(a: &Value, b: &Value) -> bool {
    let n = numbers_agree;
    match (*a, *b) {
        (Value::Point { x, y }, Value::Point { x: p, y: q })
        | (Value::Vector { x, y }, Value::Vector { x: p, y: q }) => n(x, p) && n(y, q),
        (Value::Line { a, b, c }, Value::Line { a: d, b: e, c: f }) => {
            n(a, d) && n(b, e) && n(c, f)
        }
        (
            Value::Segment { x1, y1, x2, y2 },
            Value::Segment {
                x1: p1,
                y1: q1,
                x2: p2,
                y2: q2,
            },
        ) => n(x1, p1) && n(y1, q1) && n(x2, p2) && n(y2, q2),
        (Value::Circle { cx, cy, r }, Value::Circle { cx: p, cy: q, r: s }) => {
            n(cx, p) && n(cy, q) && n(r, s)
        }
        (Value::Number(x), Value::Number(y)) => n(x, y),
        (a, b) => a == b,
    }
}

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
    /// `case:label` of every skipped object.
    skipped_labels: BTreeSet<String>,
    /// `case:label` of every object whose computed value differs from the
    /// value saved in its file.
    mismatched: BTreeMap<String, (String, String)>,
}

fn compare_case(
    case: &Case,
    bytes: &[u8],
    gaps: &gaps::Gaps,
    import_gaps: &[ImportGap],
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
    for m in &imported.mismatched {
        totals.mismatched.insert(
            format!("{}:{}", case.id, m.label),
            (m.saved.clone(), m.computed.clone()),
        );
    }
    // What the kernel gives for the golden script itself.
    let direct = geo_kernel::script::load(&case.script.join("\n")).ok();
    // The command names of the outputs the file left without a label.
    let mut unlabelled_kinds: Vec<String> = Vec::new();
    for s in &imported.skipped {
        if !s.label.is_empty() {
            totals
                .skipped_labels
                .insert(format!("{}:{}", case.id, s.label));
        }
        if s.label.is_empty() {
            unlabelled_kinds.push(s.kind.clone());
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
    let imported_mismatched: Vec<(String, String)> = imported
        .mismatched
        .iter()
        .map(|m| (m.label.clone(), m.saved.clone()))
        .collect();
    let gap = gaps.get(&case.id);
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
            // It must match by command: the script line that defines the
            // object (or, for a generated label such as `t_{1}`, the line of
            // its prefix) names the command of an output-less entry.
            let prefix = label.split("_{").next().unwrap_or(label);
            let command = case.script.iter().find_map(|l| {
                let (lhs, rhs) = l.split_once('=')?;
                let lhs = lhs.trim();
                (lhs == label || lhs == prefix).then(|| rhs.split('(').next().unwrap_or("").trim())
            });
            let slot = command.and_then(|name| unlabelled_kinds.iter().position(|k| k == name));
            if let (true, Some(slot)) = (expected.data.is_none(), slot) {
                unlabelled_kinds.remove(slot);
                totals.unlabelled_objects += 1;
            } else {
                failures.push(format!(
                    "{}: `{label}` was neither imported nor skipped",
                    case.id
                ));
            }
            continue;
        };
        let vertices: Vec<_> = c.vertices(id).collect();
        match compare_object(expected, c.kind(id), &c.value(id), &vertices) {
            Ok(_) => totals.objects_compared += 1,
            Err(why) => {
                let key = format!("0:{label}");
                let pin = import_gaps
                    .iter()
                    .find(|g| g.id == case.id && g.label == *label);
                if let Some(pin) = pin {
                    // Pinned on both sides: the golden value, the imported
                    // value, and that the golden value is what the file saved.
                    let golden = matches!(
                        expected.data,
                        Some(ExpectedData::Number(value)) if numbers_agree(value, pin.golden)
                    );
                    let imported = matches!(
                        c.value(id),
                        Value::Number(v) if numbers_agree(v, pin.imported)
                    );
                    let saved = imported_saved_number(&imported_mismatched, label)
                        .is_some_and(|s| numbers_agree(s, pin.golden));
                    if golden && imported && saved && !pin.reason.is_empty() {
                        used_import_gaps.insert(format!("{}:{label}", case.id));
                    } else {
                        failures.push(format!(
                            "{}: `{label}` no longer matches its pin in \
                             known-import-gaps.toml: {why}; expected {expected:?}, got {:?}",
                            case.id,
                            c.value(id)
                        ));
                    }
                } else if gap.is_some_and(|g| g.mismatches.contains(&key)) {
                    // A kernel gap: the importer must add nothing to it, so
                    // the imported value is what the kernel gives for the
                    // golden script itself.
                    let same = direct
                        .as_ref()
                        .and_then(|d| d.id(label).map(|i| same_value(&d.value(i), &c.value(id))));
                    if same == Some(true) {
                        totals.gap_objects += 1;
                    } else {
                        failures.push(format!(
                            "{}: `{label}` is a known kernel gap, but the import gives {:?}, \
                             the golden script {:?}",
                            case.id,
                            c.value(id),
                            direct
                                .as_ref()
                                .and_then(|d| d.id(label).map(|i| d.value(i)))
                        ));
                    }
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
    // Objects the import made that the snapshot does not have: only the
    // sides and new vertices of a polygon and the like, which the kernel
    // labels itself, and the image points of transformed segments, and only
    // defined ones (GeoGebra labels no other).
    for id in c.ids() {
        let label = c.label(id);
        if case.snapshots[0].contains_key(label) {
            continue;
        }
        let polygon_family = case.script.iter().any(|l| {
            ["Polygon(", "Vertex(", "InteriorAngles(", "Angle("]
                .iter()
                .any(|f| l.contains(f))
        });
        // the image points of a transformed segment or ray (`A'`, `B'`, or default
        // labels such as `C`)
        let image = matches!(case.group.as_str(), "transform" | "vector");
        if !(polygon_family || image) || !c.value(id).is_defined() {
            failures.push(format!(
                "{}: the import made `{label}`, which the snapshot does not have",
                case.id
            ));
        }
    }
    imported.script
}

/// The number in the `saved` text (`value N`) of the mismatch of `label`.
fn imported_saved_number(mismatches: &[(String, String)], label: &str) -> Option<f64> {
    let (_, saved) = mismatches.iter().find(|(l, _)| l == label)?;
    saved.strip_prefix("value ")?.parse().ok()
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
    let gap_file: ImportGapFile =
        toml::from_str(include_str!("../known-import-gaps.toml")).unwrap();
    let import_gaps = gap_file.gap;
    let reclassified: BTreeMap<String, &Reclassified> = gap_file
        .reclassified
        .iter()
        .map(|r| (format!("{}:{}", r.id, r.label), r))
        .collect();
    assert_eq!(
        reclassified.len(),
        gap_file.reclassified.len(),
        "a reclassified object is listed twice"
    );
    let mut used_import_gaps = BTreeSet::new();
    let mut xml = Vec::new();
    files(&root.join("golden/ggb"), "xml", &mut xml);
    // `saved/` holds files saved after moves, with their own snapshots
    // (tests/saved.rs); they have no golden case.
    xml.retain(|p| {
        p.parent()
            .and_then(|d| d.file_name())
            .is_none_or(|d| d != "saved")
    });
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
        ["polygon", "vector"]
    );
    assert!(
        totals.unlabelled <= MAX_UNLABELLED,
        "{} unlabelled",
        totals.unlabelled
    );
    assert_eq!(totals.unlabelled, totals.unlabelled_objects);
    let listed: BTreeSet<String> = import_gaps
        .iter()
        .map(|g| format!("{}:{}", g.id, g.label))
        .collect();
    assert_eq!(listed.len(), import_gaps.len(), "an entry is listed twice");
    assert_eq!(
        used_import_gaps, listed,
        "known-import-gaps.toml is stale or incomplete"
    );
    let expected_skipped: BTreeSet<String> =
        EXPECTED_SKIPPED.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        totals.skipped_labels, expected_skipped,
        "the set of skipped objects changed"
    );
    // Every disagreement with the saved value is documented: a pinned import
    // gap, a kernel gap at step 0 (whose golden value is the saved one), or a
    // pinned saved matrix that GeoGebra reclassifies on loading (the imported
    // value agreed with the golden one above, or the test already failed).
    let mut undocumented = Vec::new();
    for key in totals.mismatched.keys() {
        let (id, label) = key.split_once(':').unwrap();
        let in_kernel_gap = gaps
            .get(id)
            .is_some_and(|g| g.mismatches.contains(&format!("0:{label}")));
        let reclassified_as_pinned = reclassified
            .get(key)
            .is_some_and(|r| r.saved == totals.mismatched[key].0 && !r.reason.is_empty());
        if !(listed.contains(key) || in_kernel_gap || reclassified_as_pinned) {
            undocumented.push(format!("{key}: {:?}", totals.mismatched[key]));
        }
    }
    assert!(
        undocumented.is_empty(),
        "{} objects differ from the value saved in their file and are not documented:\n{}",
        undocumented.len(),
        undocumented.join("\n")
    );
    for key in listed.iter().chain(reclassified.keys()) {
        assert!(
            totals.mismatched.contains_key(key),
            "{key} no longer mismatches"
        );
    }
    println!(
        "objects that differ from the saved value: {}",
        totals.mismatched.len()
    );
    assert!(totals.files >= MIN_FILES, "only {} files", totals.files);
    assert!(totals.zips >= MIN_ZIPS, "only {} zip files", totals.zips);
    assert!(
        totals.objects_compared >= MIN_COMPARED,
        "only {} objects compared",
        totals.objects_compared
    );
}
